use crate::util::resolve_workspace_root;
use onaji_core::cache::{ExecutionCache, TaskState};
use onaji_core::config::ConfigLoader;
use onaji_core::dashboard::DashboardServer;
use onaji_core::graph::ImpactAnalyzer;
use onaji_core::scheduler::TaskScheduler;
use onaji_core::watcher::ReactiveWatcher;
use std::path::Path;
use std::time::Duration;

pub async fn handle_watch(
    config_path: &Path,
    debounce_ms: u64,
    tests_only: bool,
    extra_env: Vec<(String, String)>,
    dashboard_port: Option<u16>,
) {
    let workspace_root = resolve_workspace_root(config_path);
    let (manifest, dag) = match ConfigLoader::load_from_file(config_path) {
        Ok(res) => res,
        Err(err) => {
            eprintln!("✗ Error loading '{}': {}", config_path.display(), err);
            std::process::exit(1);
        }
    };

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                 ONAJI REACTIVE DAEMON                         ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!("Project:    {}", manifest.name);
    println!(
        "Workspace:  {}",
        workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_path_buf())
            .display()
    );
    println!("Resources:  {} registered", dag.resource_indices.len());
    println!("Tasks:      {} configured in DAG", dag.task_indices.len());
    println!("Debounce:   {} ms", debounce_ms);
    if tests_only {
        println!("Mode:       🧪 Smart Test Selection (--tests enabled)");
    }
    if !extra_env.is_empty() {
        println!(
            "Env:        {} custom variable(s) injected",
            extra_env.len()
        );
    }
    if let Some(port) = dashboard_port {
        println!(
            "Dashboard:  📊 Real-time UI live at http://localhost:{}",
            port
        );
    }
    println!("\nWatching for changes... (Press Ctrl+C to stop)\n");

    let (event_sender, _dash_handle) = if let Some(port) = dashboard_port {
        let server = DashboardServer::new(port).with_workspace_root(&workspace_root);
        let sender = server.event_sender();
        let srv = server.clone();
        let handle = tokio::spawn(async move {
            let _ = srv.run().await;
        });
        (Some(sender), Some(handle))
    } else {
        (None, None)
    };

    let extra_env_map: std::sync::Arc<std::collections::HashMap<String, String>> =
        std::sync::Arc::new(extra_env.into_iter().collect());

    let watcher = ReactiveWatcher::new(&workspace_root);
    let mut rx = match watcher.start_watching_with_manifest(
        &dag,
        Some(config_path),
        Duration::from_millis(debounce_ms),
    ) {
        Ok(receiver) => receiver,
        Err(err) => {
            eprintln!("✗ Failed to start filesystem watcher: {}", err);
            std::process::exit(1);
        }
    };

    let mut dag = std::sync::Arc::new(dag);
    let workspace_root = std::sync::Arc::new(workspace_root);
    let cache = std::sync::Arc::new(tokio::sync::Mutex::new(ExecutionCache::load_or_new(
        &workspace_root,
    )));
    let mut active_run: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        let event = tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c() => {
                println!("\n🛑 [watch] Received interrupt signal (Ctrl+C) — terminating active tasks...");
                if let Some(handle) = active_run.take() {
                    handle.abort();
                    let _ = handle.await;
                }
                if let Some(dh) = &_dash_handle {
                    dh.abort();
                }
                let cache_guard = cache.lock().await;
                let _ = cache_guard.save(&workspace_root);
                println!("✓ Flushed cache state. Daemon stopped cleanly.");
                break;
            }
            ev_opt = rx.recv() => {
                match ev_opt {
                    Some(ev) => ev,
                    None => break,
                }
            }
        };

        if let Some(handle) = active_run.take() {
            if !handle.is_finished() {
                println!("⚡ [watch] In-flight pipeline preempted — aborting prior tasks...");
                handle.abort();
                let _ = handle.await;
            }
        }

        if event.resource_id.as_str() == onaji_core::watcher::MANIFEST_RESOURCE_ID {
            println!("🔄 [watch] Configuration manifest changed — hot-reloading DAG...");
            match ConfigLoader::load_from_file(config_path) {
                Ok((new_manifest, new_dag)) => {
                    println!(
                        "✓ Successfully reloaded manifest '{}': {} tasks, {} resources.",
                        new_manifest.name,
                        new_dag.task_indices.len(),
                        new_dag.resource_indices.len()
                    );
                    dag = std::sync::Arc::new(new_dag);
                }
                Err(err) => {
                    eprintln!(
                        "✗ Syntax or semantic error in manifest '{}': {}. Keeping previous DAG.",
                        config_path.display(),
                        err
                    );
                }
            }
            continue;
        }

        println!("---------------------------------------------------------------");
        println!(
            "⚡ [watch] Resource '{}' {:?} (payload hash: {})",
            event.resource_id,
            event.event_type,
            event
                .payload_hash
                .map(|h| format!("{:016x}", h))
                .unwrap_or_else(|| "none".to_string())
        );

        let analyzer = ImpactAnalyzer::new(&dag);
        if let Some(raw_impact) = analyzer.analyze_resource_change(&event.resource_id) {
            let impact = if tests_only {
                raw_impact.filter_for_tests(&dag)
            } else {
                raw_impact
            };

            if tests_only {
                println!(
                    "🧪 [test-mode] {} causal test/prerequisite tasks selected, {} skipped",
                    impact.affected_tasks.len(),
                    impact.unaffected_tasks.len()
                );
            } else {
                println!(
                    "🎯 [impact] {} affected tasks, {} unaffected",
                    impact.affected_tasks.len(),
                    impact.unaffected_tasks.len()
                );
            }

            if impact.affected_tasks.is_empty() {
                println!("  ○ No test-related tasks impacted by this change.\n");
                continue;
            }

            let task_dag = std::sync::Arc::clone(&dag);
            let task_root = std::sync::Arc::clone(&workspace_root);
            let task_cache = std::sync::Arc::clone(&cache);
            let task_env = std::sync::Arc::clone(&extra_env_map);
            let task_event_tx = event_sender.clone();

            active_run = Some(tokio::spawn(async move {
                let mut cache_guard = task_cache.lock().await;
                let mut scheduler = TaskScheduler::new(&task_dag, &*task_root, &mut cache_guard);
                if let Some(tx) = task_event_tx {
                    scheduler = scheduler.with_event_sender(tx);
                }
                if !task_env.is_empty() {
                    scheduler = scheduler.with_env((*task_env).clone());
                }
                match scheduler.execute_impact(&impact).await {
                    Ok(summary) => {
                        println!("\nExecution Plan:");
                        for res in summary.results {
                            match res.state {
                                TaskState::Success => {
                                    println!("  ✓ {:<26} {:>6} ms", res.task_id, res.duration_ms);
                                }
                                TaskState::Cached => {
                                    println!("  ⚡ {:<26}   cached", res.task_id);
                                }
                                TaskState::Blocked => {
                                    println!("  ○ {:<26}  blocked", res.task_id);
                                }
                                TaskState::Failed => {
                                    println!(
                                        "  ✗ {:<26}   failed (exit code: {:?})",
                                        res.task_id, res.exit_code
                                    );
                                    if !res.stderr.is_empty() {
                                        for line in res.stderr.lines() {
                                            println!("      | {}", line);
                                        }
                                    }
                                }
                                _ => {
                                    println!("  ? {:<26}  {:?}", res.task_id, res.state);
                                }
                            }
                        }
                        for unaffected in &impact.unaffected_tasks {
                            println!("  ○ {:<26}  skipped (unaffected)", unaffected);
                        }
                        println!(
                            "\nTotal Time: {} ms | Status: {}\n",
                            summary.total_duration_ms,
                            if summary.success { "PASSED" } else { "FAILED" }
                        );
                        let _ = cache_guard.save(&task_root);
                    }
                    Err(err) => {
                        eprintln!("✗ Execution error: {}", err);
                    }
                }
            }));
        }
    }
}

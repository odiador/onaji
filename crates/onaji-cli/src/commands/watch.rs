use crate::util::resolve_workspace_root;
use onaji_core::cache::{ExecutionCache, TaskState};
use onaji_core::config::ConfigLoader;
use onaji_core::graph::ImpactAnalyzer;
use onaji_core::scheduler::TaskScheduler;
use onaji_core::watcher::ReactiveWatcher;
use std::path::Path;
use std::time::Duration;

pub async fn handle_watch(config_path: &Path, debounce_ms: u64, tests_only: bool) {
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
    println!("\nWatching for changes... (Press Ctrl+C to stop)\n");

    let watcher = ReactiveWatcher::new(&workspace_root);
    let mut rx = match watcher.start_watching(&dag, Duration::from_millis(debounce_ms)) {
        Ok(receiver) => receiver,
        Err(err) => {
            eprintln!("✗ Failed to start filesystem watcher: {}", err);
            std::process::exit(1);
        }
    };

    let dag = std::sync::Arc::new(dag);
    let workspace_root = std::sync::Arc::new(workspace_root);
    let cache = std::sync::Arc::new(tokio::sync::Mutex::new(ExecutionCache::load_or_new(
        &workspace_root,
    )));
    let mut active_run: Option<tokio::task::JoinHandle<()>> = None;

    while let Some(event) = rx.recv().await {
        if let Some(handle) = active_run.take() {
            if !handle.is_finished() {
                println!("⚡ [watch] In-flight pipeline preempted — aborting prior tasks...");
                handle.abort();
                let _ = handle.await;
            }
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

            active_run = Some(tokio::spawn(async move {
                let mut cache_guard = task_cache.lock().await;
                let mut scheduler = TaskScheduler::new(&task_dag, &*task_root, &mut cache_guard);
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

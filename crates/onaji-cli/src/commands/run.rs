use crate::cli::OutputFormat;
use crate::util::resolve_workspace_root;
use onaji_core::cache::{ExecutionCache, TaskState};
use onaji_core::config::ConfigLoader;
use onaji_core::domain::ResourceId;
use onaji_core::graph::{ImpactAnalyzer, ImpactReport};
use onaji_core::scheduler::TaskScheduler;
use std::path::{Path, PathBuf};

#[allow(clippy::too_many_arguments)]
pub async fn handle_run(
    config_path: &Path,
    resource_filter: Option<String>,
    since_filter: Option<String>,
    tests_filter: bool,
    _ci: bool,
    output: OutputFormat,
    fail_fast: bool,
    trace: bool,
    trace_format: OutputFormat,
    trace_output: Option<PathBuf>,
    remote: bool,
    workers: Vec<String>,
    extra_env: Vec<(String, String)>,
) {
    let workspace_root = resolve_workspace_root(config_path);
    let (_manifest, dag) = match ConfigLoader::load_from_file(config_path) {
        Ok(res) => res,
        Err(err) => {
            match output {
                OutputFormat::Github => {
                    println!(
                        "::error title=Onaji Manifest Error::Failed to load manifest '{}': {}",
                        config_path.display(),
                        err
                    );
                }
                OutputFormat::Json => {
                    let json = serde_json::json!({
                        "success": false,
                        "error": format!("Failed to load manifest '{}': {}", config_path.display(), err),
                        "exit_code": 2
                    });
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                OutputFormat::Text => {
                    eprintln!("✗ Error loading '{}': {}", config_path.display(), err);
                }
            }
            std::process::exit(2);
        }
    };

    let collector = onaji_core::telemetry::InMemoryTraceCollector::new();
    let is_tracing = trace
        || std::env::var("ONAJI_TRACE")
            .map(|v| v == "1")
            .unwrap_or(false);

    let root_span_id = if is_tracing {
        use onaji_core::telemetry::TelemetryCollector;
        let id = collector.start_span(
            "pipeline_root",
            None,
            "onaji.pipeline.run",
            onaji_core::telemetry::SpanKind::PipelineRun,
        );
        if let Some(r) = &resource_filter {
            collector.set_attribute(&id, "causal.trigger", r);
        }
        collector.set_attribute(&id, "workspace.root", &workspace_root.to_string_lossy());
        Some(id)
    } else {
        None
    };

    let backend_opt: Option<std::sync::Arc<dyn onaji_core::remote::ExecutionBackend>> =
        if remote || !workers.is_empty() {
            let coordinator = std::sync::Arc::new(onaji_core::remote::ClusterCoordinator::new());
            if !workers.is_empty() {
                for (idx, addr) in workers.iter().enumerate() {
                    let id = format!("remote-worker-{}", idx + 1);
                    coordinator.register_tcp_worker(
                        &id,
                        addr,
                        std::collections::HashMap::from([
                            ("rust".to_string(), "installed".to_string()),
                            ("sh".to_string(), "installed".to_string()),
                            ("android_sdk".to_string(), "installed".to_string()),
                        ]),
                    );
                    if output == OutputFormat::Text {
                        println!(
                            "⚡ [remote] Registered remote worker node '{}' at {}",
                            id, addr
                        );
                    }
                }
            } else {
                let worker_sandbox = workspace_root.join(".onaji").join("worker_sandbox");
                let worker = std::sync::Arc::new(onaji_core::remote::WorkerDaemon::new(
                    "worker-local",
                    worker_sandbox,
                    std::collections::HashMap::from([
                        ("rust".to_string(), "installed".to_string()),
                        ("sh".to_string(), "installed".to_string()),
                        ("android_sdk".to_string(), "installed".to_string()),
                    ]),
                ));
                coordinator.register_worker(worker);
            }
            if output == OutputFormat::Text {
                println!(
                "⚡ [remote] Dispatching tasks through cluster coordinator ({} worker(s) online).",
                coordinator.list_workers().len()
            );
            }
            Some(std::sync::Arc::new(
                onaji_core::remote::RemoteExecutionBackend::new(coordinator),
            ))
        } else {
            None
        };

    let mut cache = ExecutionCache::load_or_new(&workspace_root);
    let mut scheduler =
        TaskScheduler::new(&dag, &workspace_root, &mut cache).with_fail_fast(fail_fast);
    if let Some(backend) = backend_opt {
        scheduler = scheduler.with_backend(backend);
    }
    let env_map: std::collections::HashMap<String, String> = extra_env.into_iter().collect();
    if !env_map.is_empty() {
        scheduler = scheduler.with_env(env_map);
    }

    let tasks_to_run: Vec<onaji_core::domain::TaskId> = if let Some(res_str) = resource_filter {
        let res_id = ResourceId::new(res_str);
        let analyzer = ImpactAnalyzer::new(&dag);
        match analyzer.analyze_resource_change(&res_id) {
            Some(report) => {
                if tests_filter {
                    report.filter_for_tests(&dag).affected_tasks
                } else {
                    report.affected_tasks
                }
            }
            None => {
                match output {
                    OutputFormat::Github => {
                        println!(
                            "::error title=Onaji Resource Error::Resource '{}' not found in DAG",
                            res_id
                        );
                    }
                    OutputFormat::Json => {
                        let json = serde_json::json!({
                            "success": false,
                            "error": format!("Resource '{}' not found in DAG", res_id),
                            "exit_code": 3
                        });
                        println!("{}", serde_json::to_string_pretty(&json).unwrap());
                    }
                    OutputFormat::Text => {
                        eprintln!("✗ Resource '{}' not found in DAG.", res_id);
                    }
                }
                std::process::exit(3);
            }
        }
    } else if let Some(since_ref) = since_filter {
        let analyzer = ImpactAnalyzer::new(&dag);
        let changed = match onaji_core::detect_git_changes(&workspace_root, &since_ref, true) {
            Ok(f) => f,
            Err(err) => {
                eprintln!("✗ Git diff failed: {}", err);
                std::process::exit(1);
            }
        };
        let matched = analyzer.match_changed_files(&workspace_root, &changed);
        let report = analyzer.analyze_multiple_resources(&matched);
        if tests_filter {
            report.filter_for_tests(&dag).affected_tasks
        } else {
            report.affected_tasks
        }
    } else if tests_filter {
        let all_tasks: Vec<onaji_core::domain::TaskId> = dag.task_indices.keys().cloned().collect();
        let dummy_report = ImpactReport {
            root_resource: ResourceId::new("workspace"),
            affected_tasks: all_tasks,
            affected_artifacts: Vec::new(),
            unaffected_tasks: Vec::new(),
        };
        dummy_report.filter_for_tests(&dag).affected_tasks
    } else {
        dag.task_indices.keys().cloned().collect()
    };

    if tasks_to_run.is_empty() {
        if output == OutputFormat::Text {
            println!("○ No matching tasks to execute. Everything is up to date.");
        } else if output == OutputFormat::Json {
            println!(
                "{}",
                serde_json::json!({ "success": true, "tasks_executed": 0 })
            );
        }
        return;
    }

    if output == OutputFormat::Github {
        println!(
            "::group::Onaji CI Pipeline Execution ({} tasks)",
            tasks_to_run.len()
        );
    } else if output == OutputFormat::Text {
        println!("Running {} tasks...", tasks_to_run.len());
    }

    match scheduler.execute_tasks(&tasks_to_run).await {
        Ok(summary) => {
            if let Some(root_id) = &root_span_id {
                use onaji_core::telemetry::TelemetryCollector;
                for res in &summary.results {
                    let task_span = collector.start_span(
                        &format!("task_{}", res.task_id),
                        Some(root_id),
                        "onaji.task.execute",
                        onaji_core::telemetry::SpanKind::TaskExecute,
                    );
                    collector.set_attribute(&task_span, "task.id", res.task_id.as_str());
                    collector.set_attribute(&task_span, "task.state", &format!("{:?}", res.state));
                    collector.set_attribute(
                        &task_span,
                        "task.duration_ms",
                        &res.duration_ms.to_string(),
                    );
                    let status = match res.state {
                        TaskState::Success => onaji_core::telemetry::SpanStatus::Ok,
                        TaskState::Cached => onaji_core::telemetry::SpanStatus::Cached,
                        TaskState::Blocked => onaji_core::telemetry::SpanStatus::Blocked,
                        TaskState::Failed => onaji_core::telemetry::SpanStatus::Error(format!(
                            "Exit code {:?}",
                            res.exit_code
                        )),
                        _ => onaji_core::telemetry::SpanStatus::Ok,
                    };
                    collector.finish_span(&task_span, status);
                }
                let root_status = if summary.success {
                    onaji_core::telemetry::SpanStatus::Ok
                } else {
                    onaji_core::telemetry::SpanStatus::Error("Pipeline failed".to_string())
                };
                collector.finish_span(root_id, root_status);

                // Save trace atomically to .onaji/traces/latest.json
                let trace_json = collector.to_json();
                let trace_dest = trace_output.unwrap_or_else(|| {
                    let traces_dir = workspace_root.join(".onaji").join("traces");
                    std::fs::create_dir_all(&traces_dir).ok();
                    traces_dir.join("latest.json")
                });

                if let Some(parent) = trace_dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                    let tmp_dest = parent.join(format!(
                        ".{}.{}.tmp",
                        trace_dest.file_name().unwrap_or_default().to_string_lossy(),
                        std::process::id()
                    ));
                    if std::fs::write(&tmp_dest, &trace_json).is_ok() {
                        let _ = std::fs::rename(&tmp_dest, &trace_dest);
                    }
                } else {
                    let _ = std::fs::write(&trace_dest, &trace_json);
                }

                if trace_format == OutputFormat::Json {
                    println!("{}", trace_json);
                }
            }

            match output {
                OutputFormat::Github => {
                    for res in &summary.results {
                        match res.state {
                            TaskState::Success => {
                                println!(
                                    "::notice title=Task Succeeded::{:<24} ({}ms)",
                                    res.task_id, res.duration_ms
                                );
                            }
                            TaskState::Cached => {
                                println!(
                                    "::notice title=Task Cached::{:<24} ⚡ (incremental cache hit)",
                                    res.task_id
                                );
                            }
                            TaskState::Blocked => {
                                println!(
                                    "::warning title=Task Blocked::{:<24} ○ (upstream dependency failed)",
                                    res.task_id
                                );
                            }
                            TaskState::Failed => {
                                println!(
                                    "::error title=Task Failed::{:<24} ✗ (exit code: {:?})",
                                    res.task_id, res.exit_code
                                );
                                if !res.stderr.is_empty() {
                                    for line in res.stderr.lines() {
                                        println!("      | {}", line);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    println!("::endgroup::");
                    if summary.success {
                        println!(
                            "::notice title=Onaji CI::All {} tasks completed successfully in {}ms",
                            summary.results.len(),
                            summary.total_duration_ms
                        );
                    } else {
                        println!("::error title=Onaji CI::Pipeline execution failed");
                    }
                }
                OutputFormat::Json => {
                    let json = serde_json::json!({
                        "success": summary.success,
                        "total_duration_ms": summary.total_duration_ms,
                        "results": summary.results.iter().map(|r| {
                            serde_json::json!({
                                "task_id": r.task_id.as_str(),
                                "state": format!("{:?}", r.state),
                                "duration_ms": r.duration_ms,
                                "exit_code": r.exit_code,
                            })
                        }).collect::<Vec<_>>()
                    });
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                OutputFormat::Text => {
                    for res in &summary.results {
                        match res.state {
                            TaskState::Success => {
                                println!("  ✓ {:<26} {:>6} ms", res.task_id, res.duration_ms)
                            }
                            TaskState::Cached => println!("  ⚡ {:<26}   cached", res.task_id),
                            TaskState::Blocked => println!("  ○ {:<26}  blocked", res.task_id),
                            TaskState::Failed => println!(
                                "  ✗ {:<26}   failed (exit code: {:?})",
                                res.task_id, res.exit_code
                            ),
                            _ => println!("  ? {:<26}  {:?}", res.task_id, res.state),
                        }
                    }
                    println!(
                        "\nCompleted in {} ms | Status: {}",
                        summary.total_duration_ms,
                        if summary.success { "PASSED" } else { "FAILED" }
                    );
                }
            }

            let _ = cache.save(&workspace_root);
            if !summary.success {
                std::process::exit(1);
            }
        }
        Err(err) => {
            match output {
                OutputFormat::Github => {
                    println!(
                        "::error title=Onaji Execution Error::Execution error: {}",
                        err
                    );
                }
                OutputFormat::Json => {
                    let json = serde_json::json!({
                        "success": false,
                        "error": format!("Execution error: {}", err),
                        "exit_code": 1
                    });
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                OutputFormat::Text => {
                    eprintln!("✗ Execution error: {}", err);
                }
            }
            std::process::exit(1);
        }
    }
}

pub fn handle_explain(config_path: &Path, task_id: &str) {
    let workspace_root = resolve_workspace_root(config_path);
    let trace_file = workspace_root
        .join(".onaji")
        .join("traces")
        .join("latest.json");
    if !trace_file.exists() {
        eprintln!(
            "No trace telemetry found in '{}'. Run 'onaji run --trace' first.",
            workspace_root.display()
        );
        std::process::exit(1);
    }

    let trace_content = std::fs::read_to_string(&trace_file).unwrap_or_default();
    let spans: Vec<onaji_core::telemetry::SpanRecord> =
        serde_json::from_str(&trace_content).unwrap_or_default();

    println!(
        "=== Causal Telemetry Explanation for Task '{}' ===",
        task_id
    );
    let task_span = spans.iter().find(|s| {
        s.kind == onaji_core::telemetry::SpanKind::TaskExecute
            && s.attributes.get("task.id").map(|s| s.as_str()) == Some(task_id)
    });

    match task_span {
        Some(span) => {
            println!("  • State:       {:?}", span.status);
            println!(
                "  • Duration:    {} ms",
                span.duration_ms.unwrap_or_default()
            );
            for (k, v) in &span.attributes {
                println!("  • {:<12} {}", format!("{}:", k), v);
            }
        }
        None => {
            println!(
                "  Task '{}' was not executed in the latest recorded pipeline run.",
                task_id
            );
        }
    }
}

pub fn handle_impact(config_path: &Path, resource_id_str: &str, _ci: bool, output: OutputFormat) {
    let (_, dag) = match ConfigLoader::load_from_file(config_path) {
        Ok(res) => res,
        Err(err) => {
            match output {
                OutputFormat::Github => {
                    println!(
                        "::error title=Onaji Manifest Error::Failed to load manifest '{}': {}",
                        config_path.display(),
                        err
                    );
                }
                OutputFormat::Json => {
                    let json = serde_json::json!({
                        "success": false,
                        "error": format!("Failed to load manifest '{}': {}", config_path.display(), err),
                        "exit_code": 2
                    });
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                OutputFormat::Text => {
                    eprintln!("✗ Error loading '{}': {}", config_path.display(), err);
                }
            }
            std::process::exit(2);
        }
    };

    let res_id = ResourceId::new(resource_id_str);
    let analyzer = ImpactAnalyzer::new(&dag);
    match analyzer.analyze_resource_change(&res_id) {
        Some(report) => match output {
            OutputFormat::Github => {
                println!(
                    "::notice title=Onaji Impact::Resource '{}' impacts {} tasks and {} artifacts",
                    resource_id_str,
                    report.affected_tasks.len(),
                    report.affected_artifacts.len()
                );
                for t in &report.affected_tasks {
                    println!("::notice title=Affected Task::{}", t);
                }
            }
            OutputFormat::Json => {
                let json = serde_json::json!({
                    "resource": resource_id_str,
                    "affected_tasks": report.affected_tasks.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
                    "affected_artifacts": report.affected_artifacts.iter().map(|a| a.as_str()).collect::<Vec<_>>(),
                    "unaffected_tasks": report.unaffected_tasks.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&json).unwrap());
            }
            OutputFormat::Text => {
                println!(
                    "=== Impact Analysis for Resource: '{}' ===",
                    resource_id_str
                );
                println!("\nAffected Tasks ({}):", report.affected_tasks.len());
                for (idx, task) in report.affected_tasks.iter().enumerate() {
                    println!("  {}. [Task: {}]", idx + 1, task);
                }

                println!(
                    "\nAffected Artifacts ({}):",
                    report.affected_artifacts.len()
                );
                for art in &report.affected_artifacts {
                    println!("  • {}", art);
                }

                println!("\nUnaffected Tasks ({}):", report.unaffected_tasks.len());
                for task in &report.unaffected_tasks {
                    println!("  • [Task: {}]", task);
                }
            }
        },
        None => {
            match output {
                OutputFormat::Github => {
                    println!(
                        "::error title=Onaji Resource Error::Resource '{}' is not registered in the graph",
                        resource_id_str
                    );
                }
                OutputFormat::Json => {
                    let json = serde_json::json!({
                        "success": false,
                        "error": format!("Resource '{}' is not registered in the graph", resource_id_str),
                        "exit_code": 3
                    });
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                OutputFormat::Text => {
                    eprintln!(
                        "✗ Resource '{}' is not registered in the graph.",
                        resource_id_str
                    );
                }
            }
            std::process::exit(3);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_affected(
    config_path: &Path,
    since_ref: &str,
    include_uncommitted: bool,
    tests_only: bool,
    execute_run: bool,
    fail_fast: bool,
    output: OutputFormat,
    remote: bool,
    workers: Vec<String>,
    extra_env: Vec<(String, String)>,
) {
    let workspace_root = resolve_workspace_root(config_path);
    let (_manifest, dag) = match ConfigLoader::load_from_file(config_path) {
        Ok(res) => res,
        Err(err) => {
            eprintln!("✗ Error loading '{}': {}", config_path.display(), err);
            std::process::exit(2);
        }
    };

    let changed_files =
        match onaji_core::detect_git_changes(&workspace_root, since_ref, include_uncommitted) {
            Ok(f) => f,
            Err(err) => {
                eprintln!("✗ Git diff failed: {}", err);
                std::process::exit(1);
            }
        };

    let analyzer = ImpactAnalyzer::new(&dag);
    let matched_resources = analyzer.match_changed_files(&workspace_root, &changed_files);

    let raw_report = analyzer.analyze_multiple_resources(&matched_resources);
    let report = if tests_only {
        raw_report.filter_for_tests(&dag)
    } else {
        raw_report
    };

    if !execute_run {
        match output {
            OutputFormat::Github => {
                println!(
                    "::notice title=Onaji Affected::{} resources, {} tasks affected since {}",
                    matched_resources.len(),
                    report.affected_tasks.len(),
                    since_ref
                );
                for t in &report.affected_tasks {
                    println!("::notice title=Affected Task::{}", t);
                }
            }
            OutputFormat::Json => {
                let json = serde_json::json!({
                    "since": since_ref,
                    "changed_files": changed_files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                    "matched_resources": matched_resources.iter().map(|r| r.as_str()).collect::<Vec<_>>(),
                    "affected_tasks": report.affected_tasks.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
                    "affected_artifacts": report.affected_artifacts.iter().map(|a| a.as_str()).collect::<Vec<_>>(),
                    "unaffected_tasks": report.unaffected_tasks.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&json).unwrap());
            }
            OutputFormat::Text => {
                println!("=== Onaji Affected Analysis (since '{}') ===", since_ref);
                println!("Changed files:     {}", changed_files.len());
                println!("Matched resources: {}", matched_resources.len());
                for r in &matched_resources {
                    println!("  • {}", r);
                }
                println!("\nAffected Tasks ({}):", report.affected_tasks.len());
                for (idx, task) in report.affected_tasks.iter().enumerate() {
                    println!("  {}. [Task: {}]", idx + 1, task);
                }
                if !report.affected_artifacts.is_empty() {
                    println!(
                        "\nAffected Artifacts ({}):",
                        report.affected_artifacts.len()
                    );
                    for art in &report.affected_artifacts {
                        println!("  • {}", art);
                    }
                }
                println!("\nUnaffected Tasks ({}):", report.unaffected_tasks.len());
                for task in &report.unaffected_tasks {
                    println!("  • [Task: {}]", task);
                }
            }
        }
        return;
    }

    if report.affected_tasks.is_empty() {
        if output == OutputFormat::Json {
            let json = serde_json::json!({
                "success": true,
                "since": since_ref,
                "affected_tasks": [],
                "message": "No affected tasks to run"
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        } else {
            println!(
                "○ No affected tasks to run since '{}'. Everything is up to date.",
                since_ref
            );
        }
        return;
    }

    if output == OutputFormat::Text {
        println!(
            "Running {} affected tasks (since '{}')...\n",
            report.affected_tasks.len(),
            since_ref
        );
    }

    let backend_opt: Option<std::sync::Arc<dyn onaji_core::remote::ExecutionBackend>> =
        if remote || !workers.is_empty() {
            let coordinator = std::sync::Arc::new(onaji_core::remote::ClusterCoordinator::new());
            if !workers.is_empty() {
                for (idx, addr) in workers.iter().enumerate() {
                    let id = format!("remote-worker-{}", idx + 1);
                    coordinator.register_tcp_worker(
                        &id,
                        addr,
                        std::collections::HashMap::from([
                            ("rust".to_string(), "installed".to_string()),
                            ("sh".to_string(), "installed".to_string()),
                            ("android_sdk".to_string(), "installed".to_string()),
                        ]),
                    );
                }
            } else {
                let worker_sandbox = workspace_root.join(".onaji").join("worker_sandbox");
                let worker = std::sync::Arc::new(onaji_core::remote::WorkerDaemon::new(
                    "worker-local",
                    worker_sandbox,
                    std::collections::HashMap::from([
                        ("rust".to_string(), "installed".to_string()),
                        ("sh".to_string(), "installed".to_string()),
                        ("android_sdk".to_string(), "installed".to_string()),
                    ]),
                ));
                coordinator.register_worker(worker);
            }
            Some(std::sync::Arc::new(
                onaji_core::remote::RemoteExecutionBackend::new(coordinator),
            ))
        } else {
            None
        };

    let mut cache = ExecutionCache::load_or_new(&workspace_root);
    let mut scheduler =
        TaskScheduler::new(&dag, &workspace_root, &mut cache).with_fail_fast(fail_fast);
    if let Some(backend) = backend_opt {
        scheduler = scheduler.with_backend(backend);
    }
    let env_map: std::collections::HashMap<String, String> = extra_env.into_iter().collect();
    if !env_map.is_empty() {
        scheduler = scheduler.with_env(env_map);
    }
    match scheduler.execute_tasks(&report.affected_tasks).await {
        Ok(summary) => {
            match output {
                OutputFormat::Github => {
                    for res in &summary.results {
                        match res.state {
                            TaskState::Success => {
                                println!(
                                    "::notice title=Task Succeeded::{:<24} ({}ms)",
                                    res.task_id, res.duration_ms
                                );
                            }
                            TaskState::Cached => {
                                println!(
                                    "::notice title=Task Cached::{:<24} ⚡ (incremental cache hit)",
                                    res.task_id
                                );
                            }
                            TaskState::Blocked => {
                                println!(
                                    "::warning title=Task Blocked::{:<24} ○ (upstream causal failure)",
                                    res.task_id
                                );
                            }
                            TaskState::Failed => {
                                println!(
                                    "::error title=Task Failed::{:<24} (exit code: {:?})",
                                    res.task_id, res.exit_code
                                );
                                if !res.stderr.is_empty() {
                                    for line in res.stderr.lines() {
                                        println!("::error::{}", line);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                OutputFormat::Json => {
                    let results_json: Vec<_> = summary
                        .results
                        .iter()
                        .map(|r| {
                            serde_json::json!({
                                "task_id": r.task_id.as_str(),
                                "state": format!("{:?}", r.state),
                                "exit_code": r.exit_code,
                                "duration_ms": r.duration_ms,
                                "stdout": r.stdout,
                                "stderr": r.stderr,
                            })
                        })
                        .collect();
                    let json = serde_json::json!({
                        "success": summary.success,
                        "total_duration_ms": summary.total_duration_ms,
                        "since": since_ref,
                        "results": results_json,
                    });
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                OutputFormat::Text => {
                    for res in &summary.results {
                        match res.state {
                            TaskState::Success => {
                                println!("  ✓ {:<26} {:>6} ms", res.task_id, res.duration_ms)
                            }
                            TaskState::Cached => println!("  ⚡ {:<26}   cached", res.task_id),
                            TaskState::Blocked => println!("  ○ {:<26}  blocked", res.task_id),
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
                            _ => println!("  ? {:<26}  {:?}", res.task_id, res.state),
                        }
                    }
                    println!(
                        "\nTotal Time: {} ms | Status: {}\n",
                        summary.total_duration_ms,
                        if summary.success { "PASSED" } else { "FAILED" }
                    );
                }
            }
            let _ = cache.save(&workspace_root);
            if !summary.success {
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("✗ Execution error: {}", err);
            std::process::exit(1);
        }
    }
}

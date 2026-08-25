use onaji_core::domain::{Action, Task, TaskId, TaskPriority};
use onaji_core::scheduler::{DistributedTaskScheduler, TaskSchedulingState};
use std::collections::HashMap;

#[test]
fn test_distributed_scheduling_backpressure_and_priorities() {
    let mut scheduler = DistributedTaskScheduler::new();

    // Register 2 workers with 2 slots each = 4 total cluster slots
    scheduler.register_worker(
        "worker-a",
        2,
        HashMap::from([("rust".to_string(), "1.0".to_string())]),
    );
    scheduler.register_worker(
        "worker-b",
        2,
        HashMap::from([("android_sdk".to_string(), "1.0".to_string())]),
    );

    assert_eq!(scheduler.total_available_slots(), 4);

    // Add 10 tasks with varying priorities
    let p_low = Task {
        id: TaskId::new("task_low"),
        description: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "echo low".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: Vec::new(),
        priority: TaskPriority::Low,
    };

    let p_critical = Task {
        id: TaskId::new("task_critical"),
        description: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "echo critical".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: Vec::new(),
        priority: TaskPriority::Critical,
    };

    let p_high_android = Task {
        id: TaskId::new("android_compile_high"),
        description: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "echo android".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: Vec::new(),
        priority: TaskPriority::High,
    };

    scheduler.add_task(p_low);
    scheduler.add_task(p_critical);
    scheduler.add_task(p_high_android);

    // Schedule next batch
    let batch1 = scheduler.schedule_next_batch();
    assert_eq!(batch1.len(), 3);

    // Verify task_critical and android_compile_high are running
    assert_eq!(
        scheduler.tasks[&TaskId::new("task_critical")].state,
        TaskSchedulingState::Running
    );
    assert_eq!(
        scheduler.tasks[&TaskId::new("android_compile_high")].state,
        TaskSchedulingState::Running
    );
    // Verify android task assigned specifically to worker-b
    assert_eq!(
        scheduler.tasks[&TaskId::new("android_compile_high")]
            .assigned_worker
            .as_deref(),
        Some("worker-b")
    );

    // Complete task_critical -> slot frees up
    scheduler.handle_task_completion(&TaskId::new("task_critical"), true);
    assert_eq!(
        scheduler.tasks[&TaskId::new("task_critical")].state,
        TaskSchedulingState::Completed
    );
}

#[test]
fn test_backpressure_strict_slot_limit_under_heavy_load() {
    let mut scheduler = DistributedTaskScheduler::new();

    // 4 workers x 2 slots = 8 slots total
    for i in 0..4 {
        scheduler.register_worker(format!("worker-{}", i), 2, HashMap::new());
    }

    // Add 50 tasks
    for i in 0..50 {
        let task = Task {
            id: TaskId::new(format!("task_{:03}", i)),
            description: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            action: Action::Command {
                run: "echo test".to_string(),
                working_dir: None,
                env: HashMap::new(),
            },
            cache_paths: Vec::new(),
            priority: TaskPriority::Normal,
        };
        scheduler.add_task(task);
    }

    // First batch allocation
    let batch = scheduler.schedule_next_batch();
    assert_eq!(batch.len(), 8); // Strictly bounded by 8 slots
    assert_eq!(scheduler.total_running_tasks(), 8);
    assert_eq!(scheduler.total_available_slots(), 0);

    // Any further immediate schedule returns 0 (Backpressure active)
    let batch_empty = scheduler.schedule_next_batch();
    assert!(batch_empty.is_empty());
}

#[test]
fn test_capability_aware_work_stealing_and_worker_loss_requeue() {
    let mut scheduler = DistributedTaskScheduler::new();

    scheduler.register_worker(
        "worker-rust",
        1,
        HashMap::from([("rust".to_string(), "1.0".to_string())]),
    );

    let task_rust = Task {
        id: TaskId::new("rust_build"),
        description: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        action: Action::Command {
            run: "cargo build".to_string(),
            working_dir: None,
            env: HashMap::new(),
        },
        cache_paths: Vec::new(),
        priority: TaskPriority::Normal,
    };
    scheduler.add_task(task_rust);

    // Work stealing test: worker-rust steals rust_build
    let stolen = scheduler.steal_work("worker-rust");
    assert!(stolen.is_some());
    let (t_id, w_id) = stolen.unwrap();
    assert_eq!(t_id.as_str(), "rust_build");
    assert_eq!(w_id.as_str(), "worker-rust");

    // Worker loss test: worker-rust crashes
    let lost = scheduler.handle_worker_loss("worker-rust");
    assert_eq!(lost, vec![TaskId::new("rust_build")]);
    // Task is automatically re-queued for retry!
    assert_eq!(
        scheduler.tasks[&TaskId::new("rust_build")].state,
        TaskSchedulingState::Queued
    );
    assert_eq!(scheduler.tasks[&TaskId::new("rust_build")].retries, 1);
}

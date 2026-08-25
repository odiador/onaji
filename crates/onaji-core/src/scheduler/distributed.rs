use crate::domain::{Task, TaskId, TaskPriority};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskSchedulingState {
    Pending,
    Ready,
    Queued,
    Assigned,
    Running,
    Completed,
    Failed,
    Cancelled,
    Lost,
}

#[derive(Debug, Clone)]
pub struct ScheduledTaskItem {
    pub task_id: TaskId,
    pub task: Task,
    pub base_priority: TaskPriority,
    pub enqueued_at: Instant,
    pub required_toolchain: Option<String>,
    pub state: TaskSchedulingState,
    pub assigned_worker: Option<String>,
    pub retries: usize,
}

impl ScheduledTaskItem {
    pub fn new(task: Task) -> Self {
        let req_toolchain =
            if task.id.as_str().contains("android") || task.id.as_str().contains("gradle") {
                Some("android_sdk".to_string())
            } else if task.id.as_str().contains("rust") || task.id.as_str().contains("cargo") {
                Some("rust".to_string())
            } else {
                None
            };

        Self {
            task_id: task.id.clone(),
            base_priority: task.priority,
            task,
            enqueued_at: Instant::now(),
            required_toolchain: req_toolchain,
            state: TaskSchedulingState::Queued,
            assigned_worker: None,
            retries: 0,
        }
    }

    pub fn effective_priority(&self) -> u32 {
        let elapsed_secs = self.enqueued_at.elapsed().as_secs();
        let aging_bonus = (elapsed_secs * 5).min(50) as u32;
        self.base_priority.score() + aging_bonus
    }
}

#[derive(Debug, Clone)]
pub struct WorkerCapacity {
    pub worker_id: String,
    pub max_slots: usize,
    pub running_slots: usize,
    pub toolchains: HashMap<String, String>,
}

impl WorkerCapacity {
    pub fn available_slots(&self) -> usize {
        self.max_slots.saturating_sub(self.running_slots)
    }

    pub fn can_handle(&self, required_toolchain: Option<&str>) -> bool {
        match required_toolchain {
            Some(req) => self.toolchains.contains_key(req),
            None => true,
        }
    }
}

pub struct DistributedTaskScheduler {
    pub tasks: HashMap<TaskId, ScheduledTaskItem>,
    pub workers: HashMap<String, WorkerCapacity>,
    pub max_retries: usize,
}

impl Default for DistributedTaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl DistributedTaskScheduler {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            workers: HashMap::new(),
            max_retries: 3,
        }
    }

    pub fn add_task(&mut self, task: Task) {
        let item = ScheduledTaskItem::new(task);
        self.tasks.insert(item.task_id.clone(), item);
    }

    pub fn register_worker(
        &mut self,
        worker_id: impl Into<String>,
        max_slots: usize,
        toolchains: HashMap<String, String>,
    ) {
        let id = worker_id.into();
        self.workers.insert(
            id.clone(),
            WorkerCapacity {
                worker_id: id,
                max_slots,
                running_slots: 0,
                toolchains,
            },
        );
    }

    pub fn total_available_slots(&self) -> usize {
        self.workers.values().map(|w| w.available_slots()).sum()
    }

    pub fn total_running_tasks(&self) -> usize {
        self.workers.values().map(|w| w.running_slots).sum()
    }

    pub fn schedule_next_batch(&mut self) -> Vec<(TaskId, String)> {
        let mut assignments = Vec::new();

        // 1. Gather all queued candidate tasks and sort by effective priority descending
        let mut candidate_ids: Vec<TaskId> = self
            .tasks
            .iter()
            .filter(|(_, item)| item.state == TaskSchedulingState::Queued)
            .map(|(id, _)| id.clone())
            .collect();

        candidate_ids.sort_by_key(|id| {
            let item = &self.tasks[id];
            std::cmp::Reverse(item.effective_priority())
        });

        // 2. Assign each candidate to a compatible worker with available slots (Backpressure invariant)
        for task_id in candidate_ids {
            let req_toolchain = self.tasks[&task_id].required_toolchain.clone();

            // Find best matching worker with available capacity
            let selected_worker_id = self
                .workers
                .values_mut()
                .filter(|w| w.available_slots() > 0 && w.can_handle(req_toolchain.as_deref()))
                .min_by_key(|w| w.running_slots) // Load balancing: least loaded
                .map(|w| {
                    w.running_slots += 1;
                    w.worker_id.clone()
                });

            if let Some(worker_id) = selected_worker_id {
                let task_item = self.tasks.get_mut(&task_id).unwrap();
                task_item.state = TaskSchedulingState::Running;
                task_item.assigned_worker = Some(worker_id.clone());
                assignments.push((task_id, worker_id));
            }
        }

        assignments
    }

    pub fn handle_task_completion(&mut self, task_id: &TaskId, success: bool) {
        if let Some(item) = self.tasks.get_mut(task_id) {
            item.state = if success {
                TaskSchedulingState::Completed
            } else {
                TaskSchedulingState::Failed
            };

            if let Some(worker_id) = &item.assigned_worker {
                if let Some(worker) = self.workers.get_mut(worker_id) {
                    worker.running_slots = worker.running_slots.saturating_sub(1);
                }
            }
        }
    }

    pub fn handle_worker_loss(&mut self, worker_id: &str) -> Vec<TaskId> {
        let mut lost_tasks = Vec::new();
        self.workers.remove(worker_id);

        for item in self.tasks.values_mut() {
            if item.assigned_worker.as_deref() == Some(worker_id)
                && item.state == TaskSchedulingState::Running
            {
                lost_tasks.push(item.task_id.clone());
                if item.retries < self.max_retries {
                    item.retries += 1;
                    item.state = TaskSchedulingState::Queued;
                    item.assigned_worker = None;
                } else {
                    item.state = TaskSchedulingState::Lost;
                }
            }
        }

        lost_tasks
    }

    pub fn steal_work(&mut self, idle_worker_id: &str) -> Option<(TaskId, String)> {
        let idle_worker = self.workers.get(idle_worker_id)?;
        if idle_worker.available_slots() == 0 {
            return None;
        }

        let idle_toolchains = idle_worker.toolchains.clone();

        // Find candidate in Queued state compatible with idle worker
        let mut queued_candidates: Vec<TaskId> = self
            .tasks
            .iter()
            .filter(|(_, item)| {
                item.state == TaskSchedulingState::Queued
                    && item
                        .required_toolchain
                        .as_ref()
                        .map(|req| idle_toolchains.contains_key(req))
                        .unwrap_or(true)
            })
            .map(|(id, _)| id.clone())
            .collect();

        queued_candidates.sort_by_key(|id| {
            let item = &self.tasks[id];
            std::cmp::Reverse(item.effective_priority())
        });

        if let Some(target_id) = queued_candidates.into_iter().next() {
            let worker = self.workers.get_mut(idle_worker_id).unwrap();
            worker.running_slots += 1;

            let task_item = self.tasks.get_mut(&target_id).unwrap();
            task_item.state = TaskSchedulingState::Running;
            task_item.assigned_worker = Some(idle_worker_id.to_string());

            return Some((target_id, idle_worker_id.to_string()));
        }

        None
    }
}

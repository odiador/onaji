use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnajiMetrics {
    pub tasks_total: usize,
    pub tasks_success: usize,
    pub tasks_cached: usize,
    pub tasks_failed: usize,
    pub tasks_blocked: usize,
    pub cache_hits_total: usize,
    pub cache_misses_total: usize,
    pub plugin_executions_total: usize,
    pub pipeline_duration_ms: u64,
}

impl OnajiMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cache_hit_ratio(&self) -> f64 {
        let total_lookups = self.cache_hits_total + self.cache_misses_total;
        if total_lookups == 0 {
            0.0
        } else {
            self.cache_hits_total as f64 / total_lookups as f64
        }
    }

    pub fn record_task_result(&mut self, is_cached: bool, is_success: bool, is_blocked: bool) {
        self.tasks_total += 1;
        if is_cached {
            self.tasks_cached += 1;
            self.cache_hits_total += 1;
        } else if is_blocked {
            self.tasks_blocked += 1;
        } else if is_success {
            self.tasks_success += 1;
            self.cache_misses_total += 1;
        } else {
            self.tasks_failed += 1;
            self.cache_misses_total += 1;
        }
    }
}

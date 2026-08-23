pub mod cache;
pub mod config;
pub mod domain;
pub mod fingerprint;
pub mod graph;
pub mod runner;
pub mod scheduler;
pub mod watcher;

pub use cache::{CacheKey, ExecutionCache, TaskExecutionRecord, TaskState};
pub use config::{ConfigError, ConfigLoader, RawManifest};
pub use domain::*;
pub use fingerprint::{Fingerprint, Fingerprinter};
pub use graph::{DependencyGraph, GraphBuildError, GraphBuilder, ImpactAnalyzer, ImpactReport};
pub use runner::{ProcessExecutor, RunnerError, TaskExecutionResult};
pub use scheduler::{PipelineExecutionSummary, TaskScheduler};
pub use watcher::{EventNormalizer, NormalizationResult, ReactiveWatcher, WatcherError};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_valid_dag_and_impact_analysis() {
        let manifest_yaml = r#"
version: "1"
name: "test-pipeline"

resources:
  openapi_spec:
    type: file
    path: "./backend/api/openapi.yaml"
  docs_file:
    type: file
    path: "./README.md"

artifacts:
  validated_openapi:
    description: "Contract validated"
  kotlin_client:
    description: "Generated Kotlin SDK"
  android_build:
    description: "Gradle build binaries"
  docs_rendered:
    description: "Rendered HTML documentation"

tasks:
  validate_contract:
    inputs: [openapi_spec]
    outputs: [validated_openapi]
    run: "redocly lint"

  generate_kotlin:
    inputs: [validated_openapi]
    outputs: [kotlin_client]
    run: "openapi-generator"

  gradle_compile:
    inputs: [kotlin_client]
    outputs: [android_build]
    run: "./gradlew compileDebugKotlin"

  build_docs:
    inputs: [docs_file]
    outputs: [docs_rendered]
    run: "mkdocs build"
"#;

        let (manifest, dag) =
            ConfigLoader::load_from_str(manifest_yaml).expect("Failed to load valid manifest");
        assert_eq!(manifest.name, "test-pipeline");
        assert_eq!(dag.node_count(), 10);

        let analyzer = ImpactAnalyzer::new(&dag);
        let impact = analyzer
            .analyze_resource_change(&ResourceId::new("openapi_spec"))
            .expect("Expected impact report for openapi_spec");

        assert_eq!(impact.root_resource.as_str(), "openapi_spec");

        let affected_task_ids: Vec<&str> =
            impact.affected_tasks.iter().map(|t| t.as_str()).collect();
        assert_eq!(
            affected_task_ids,
            vec!["validate_contract", "generate_kotlin", "gradle_compile"]
        );

        let unaffected_task_ids: Vec<&str> =
            impact.unaffected_tasks.iter().map(|t| t.as_str()).collect();
        assert_eq!(unaffected_task_ids, vec!["build_docs"]);
    }

    #[test]
    fn test_rejects_cyclic_dependency() {
        let cyclic_yaml = r#"
version: "1"
name: "cyclic-pipeline"

resources:
  root_res:
    type: file
    path: "./root"

artifacts:
  art_a: {}
  art_b: {}
  art_c: {}

tasks:
  task_a:
    inputs: [root_res, art_c]
    outputs: [art_a]
    run: "echo a"

  task_b:
    inputs: [art_a]
    outputs: [art_b]
    run: "echo b"

  task_c:
    inputs: [art_b]
    outputs: [art_c]
    run: "echo c"
"#;

        let result = ConfigLoader::load_from_str(cyclic_yaml);
        match result {
            Err(ConfigError::Graph(GraphBuildError::CyclicDependency { .. })) => (),
            other => panic!("Expected CyclicDependency error, got {:?}", other),
        }
    }

    #[test]
    fn test_rejects_conflicting_artifact_producers() {
        let conflict_yaml = r#"
version: "1"
name: "conflict-pipeline"

resources:
  res_1:
    type: file
    path: "./res1"
  res_2:
    type: file
    path: "./res2"

artifacts:
  shared_artifact: {}

tasks:
  task_1:
    inputs: [res_1]
    outputs: [shared_artifact]
    run: "echo 1"

  task_2:
    inputs: [res_2]
    outputs: [shared_artifact]
    run: "echo 2"
"#;

        let result = ConfigLoader::load_from_str(conflict_yaml);
        match result {
            Err(ConfigError::Graph(GraphBuildError::ConflictingArtifactProducer {
                artifact_id,
                ..
            })) => {
                assert_eq!(artifact_id.as_str(), "shared_artifact");
            }
            other => panic!(
                "Expected ConflictingArtifactProducer error, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_rejects_unknown_input() {
        let unknown_input_yaml = r#"
version: "1"
name: "unknown-input-pipeline"

resources:
  real_res:
    type: file
    path: "./res"

artifacts:
  art_out: {}

tasks:
  task_1:
    inputs: [non_existent_resource]
    outputs: [art_out]
    run: "echo"
"#;

        let result = ConfigLoader::load_from_str(unknown_input_yaml);
        match result {
            Err(ConfigError::Graph(GraphBuildError::UnknownInput { input_id, .. })) => {
                assert_eq!(input_id, "non_existent_resource");
            }
            other => panic!("Expected UnknownInput error, got {:?}", other),
        }
    }

    #[test]
    fn test_rejects_duplicate_node_ids() {
        let duplicate_yaml = r#"
version: "1"
name: "duplicate-id-pipeline"

resources:
  my_name:
    type: file
    path: "./res"

artifacts:
  my_name: {}

tasks:
  task_1:
    inputs: [my_name]
    outputs: []
    run: "echo"
"#;

        let result = ConfigLoader::load_from_str(duplicate_yaml);
        match result {
            Err(ConfigError::Graph(GraphBuildError::DuplicateNodeId { id, .. })) => {
                assert_eq!(id, "my_name");
            }
            other => panic!("Expected DuplicateNodeId error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_scheduler_incremental_cache_and_blocking() {
        let temp_dir = tempfile_dir("onaji_test_scheduler");
        std::fs::create_dir_all(&temp_dir).unwrap();

        let input_file = temp_dir.join("input.txt");
        let mut f = File::create(&input_file).unwrap();
        writeln!(f, "initial content").unwrap();

        let manifest_yaml = format!(
            r#"
version: "1"
name: "scheduler-test"

resources:
  src_file:
    type: file
    path: "{}"

artifacts:
  art_1: {{}}
  art_2: {{}}

tasks:
  task_1:
    inputs: [src_file]
    outputs: [art_1]
    run: "echo task1_done"

  task_2:
    inputs: [art_1]
    outputs: [art_2]
    run: "echo task2_done"
"#,
            input_file.to_string_lossy()
        );

        let (_, dag) = ConfigLoader::load_from_str(&manifest_yaml).unwrap();
        let mut cache = ExecutionCache::new();

        let task_ids = vec![TaskId::new("task_1"), TaskId::new("task_2")];

        // 1. Primera ejecución -> Miss -> Ambas Success
        let mut scheduler = TaskScheduler::new(&dag, &temp_dir, &mut cache);
        let summary1 = scheduler.execute_tasks(&task_ids).await.unwrap();
        assert!(summary1.success);
        assert_eq!(summary1.results.len(), 2);
        assert_eq!(summary1.results[0].state, TaskState::Success);
        assert_eq!(summary1.results[1].state, TaskState::Success);

        // 2. Segunda ejecución sin cambios -> Hit -> Ambas Cached
        let mut scheduler2 = TaskScheduler::new(&dag, &temp_dir, &mut cache);
        let summary2 = scheduler2.execute_tasks(&task_ids).await.unwrap();
        assert!(summary2.success);
        assert_eq!(summary2.results[0].state, TaskState::Cached);
        assert_eq!(summary2.results[1].state, TaskState::Cached);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_event_normalizer_ignores_unchanged_touch() {
        let temp_dir = tempfile_dir("onaji_test_normalizer");
        std::fs::create_dir_all(&temp_dir).unwrap();

        let file_path = temp_dir.join("openapi.yaml");
        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "openapi: 3.0.0").unwrap();

        let res = Resource::new(
            "openapi_spec",
            ResourceType::File {
                path: file_path.clone(),
                debounce_ms: 200,
            },
        );

        let mut normalizer = EventNormalizer::new(&temp_dir);
        normalizer.register_resources(std::iter::once(&res));

        // Simular un touch (sin cambio de bytes)
        let res1 = normalizer.normalize_fs_event(&file_path);
        assert_eq!(
            res1,
            NormalizationResult::IgnoredUnchanged {
                resource_id: ResourceId::new("openapi_spec")
            }
        );

        // Simular modificación real de contenido
        {
            let mut f2 = std::fs::OpenOptions::new()
                .append(true)
                .open(&file_path)
                .unwrap();
            writeln!(f2, "info: title: Updated API").unwrap();
        }

        let res2 = normalizer.normalize_fs_event(&file_path);
        match res2 {
            NormalizationResult::Changed(event) => {
                assert_eq!(event.resource_id.as_str(), "openapi_spec");
                assert_eq!(event.event_type, EventType::Modified);
                assert!(event.payload_hash.is_some());
            }
            other => panic!("Expected Changed event, got {:?}", other),
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    fn tempfile_dir(name: &str) -> std::path::PathBuf {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{}_{}", name, now))
    }
}

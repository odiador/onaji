use onaji_core::cache::{CacheKey, ExecutionCache};
use onaji_core::daemon::Dispatcher;
use onaji_core::domain::{
    Action, Artifact, ArtifactId, InputNode, Resource, ResourceId, ResourceType, Task, TaskId,
};
use onaji_core::fingerprint::{Fingerprint, Fingerprinter};
use onaji_core::graph::GraphBuilder;
use onaji_protocol::RpcRequest;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

#[tokio::test]
async fn run_architecture_and_performance_benchmark() {
    println!("\n========================================================");
    println!("   ONΛJI SYSTEM ARCHITECTURE & PERFORMANCE BENCHMARK    ");
    println!("========================================================");

    // 1. Benchmark: DAG Graph Builder & Cycle Detection (Scale: 500 tasks, 1000 edges)
    {
        let start = Instant::now();
        let mut builder = GraphBuilder::new();
        builder = builder.add_resource(Resource::new(
            ResourceId::new("root_spec"),
            ResourceType::File {
                path: PathBuf::from("spec.json"),
                debounce_ms: 100,
            },
        ));

        for i in 0..500 {
            builder = builder.add_artifact(Artifact::new(ArtifactId::new(format!("artifact_{i}"))));
            let inputs = if i == 0 {
                vec![InputNode::Resource(ResourceId::new("root_spec"))]
            } else {
                vec![InputNode::Artifact(ArtifactId::new(format!(
                    "artifact_{}",
                    i - 1
                )))]
            };
            let outputs = vec![ArtifactId::new(format!("artifact_{i}"))];
            let task = Task::new(
                TaskId::new(format!("task_{i}")),
                inputs,
                outputs,
                Action::Command {
                    run: format!("echo step {i}"),
                    working_dir: None,
                    env: HashMap::new(),
                },
            );
            builder = builder.add_task(task);
        }

        let build_start = Instant::now();
        let dag = builder.build().expect("DAG build should succeed");
        let build_duration = build_start.elapsed();
        let total_duration = start.elapsed();

        println!("\n[1] DAG Construction & Verification (500 nodes chain):");
        println!("    - Total construction time : {:?}", total_duration);
        println!("    - Core build & toposort   : {:?}", build_duration);
        println!("    - Nodes in graph          : {}", dag.graph.node_count());
        println!("    - Edges in graph          : {}", dag.graph.edge_count());
    }

    // 2. Benchmark: Cache Key Generation & Hashing Throughput
    {
        let dummy_root = PathBuf::from(".");
        let mut dummy_fps = HashMap::new();
        for i in 0..10 {
            dummy_fps.insert(
                format!("input_{i}"),
                Fingerprint::new(0x1234567890abcdef ^ i),
            );
        }
        let task = Task::new(
            TaskId::new("benchmark_task"),
            vec![],
            vec![],
            Action::Command {
                run: "cargo build --release".to_string(),
                working_dir: None,
                env: HashMap::from([("RUSTFLAGS".to_string(), "-C opt-level=3".to_string())]),
            },
        );

        let start = Instant::now();
        let iterations = 10_000;
        for _ in 0..iterations {
            let _ = CacheKey::compute(&task, &dummy_root, &dummy_fps);
        }
        let duration = start.elapsed();
        let ops_per_sec = (iterations as f64) / duration.as_secs_f64();

        println!("\n[2] Deterministic CacheKey (SHA-256) Generation Throughput:");
        println!("    - Iterations              : {}", iterations);
        println!("    - Total time              : {:?}", duration);
        println!("    - Ops / Second            : {:.0} ops/sec", ops_per_sec);
        println!(
            "    - Mean latency per key    : {:.2} µs",
            duration.as_micros() as f64 / iterations as f64
        );
    }

    // 3. Benchmark: In-Memory ExecutionCache Operations (Lookup & Invalidation)
    {
        let mut cache = ExecutionCache::new();
        let key = CacheKey::new("test_payload_cache_key");
        let task_id = TaskId::new("task_cached");

        cache.record_success(&task_id, key.clone(), 42);

        let start = Instant::now();
        let iterations = 50_000;
        for _ in 0..iterations {
            assert!(cache.check_hit(&task_id, &key));
        }
        let duration = start.elapsed();
        let ops_per_sec = (iterations as f64) / duration.as_secs_f64();

        println!("\n[3] In-Memory CAS Cache Lookups (Hit Rate Check):");
        println!("    - Iterations              : {}", iterations);
        println!("    - Total time              : {:?}", duration);
        println!("    - Ops / Second            : {:.0} ops/sec", ops_per_sec);
        println!(
            "    - Mean lookup latency     : {:.3} µs",
            duration.as_micros() as f64 / iterations as f64
        );
    }

    // 4. Benchmark: JSON-RPC Dispatcher Handling
    {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../");
        let mut dispatcher = Dispatcher::new(workspace_root);
        let req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "graph/get".to_string(),
            params: None,
        };

        let start = Instant::now();
        let iterations = 1_000;
        for _ in 0..iterations {
            let resp = dispatcher.dispatch_request(&req).await;
            assert!(resp.error.is_none());
        }
        let duration = start.elapsed();
        let ops_per_sec = (iterations as f64) / duration.as_secs_f64();

        println!("\n[4] Daemon JSON-RPC Request Dispatch ('graph/get'):");
        println!("    - Iterations              : {}", iterations);
        println!("    - Total time              : {:?}", duration);
        println!("    - Ops / Second            : {:.0} ops/sec", ops_per_sec);
        println!(
            "    - Mean dispatch latency   : {:.2} µs",
            duration.as_micros() as f64 / iterations as f64
        );
    }

    // 5. Benchmark: xxHash3 Memory Hashing vs File I/O
    {
        let data = vec![0x42u8; 1024 * 1024]; // 1 MB buffer
        let start = Instant::now();
        let iterations = 1_000;
        for _ in 0..iterations {
            let _ = Fingerprinter::compute_bytes(&data);
        }
        let duration = start.elapsed();
        let mb_processed = (iterations as f64) * 1.0;
        let throughput_mb = mb_processed / duration.as_secs_f64();

        println!("\n[5] Content Hashing Speed (xxHash3 on 1MB payloads):");
        println!("    - Processed               : {:.0} MB", mb_processed);
        println!("    - Total time              : {:?}", duration);
        println!(
            "    - Throughput              : {:.2} MB/s ({:.2} GB/s)",
            throughput_mb,
            throughput_mb / 1024.0
        );
    }

    println!("\n========================================================\n");
}

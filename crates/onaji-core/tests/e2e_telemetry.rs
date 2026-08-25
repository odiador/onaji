use onaji_core::telemetry::{
    InMemoryTraceCollector, SpanEvent, SpanKind, SpanStatus, TelemetryCollector,
};

#[test]
fn test_causal_telemetry_spans_metrics_and_explanation() {
    let collector = InMemoryTraceCollector::new();

    // 1. Start Root Pipeline Span
    let root_span_id = collector.start_span(
        "pipeline_001",
        None,
        "onaji.pipeline.run",
        SpanKind::PipelineRun,
    );
    collector.set_attribute(&root_span_id, "causal.trigger", "openapi_spec");
    collector.set_attribute(&root_span_id, "workspace.root", "/workspace/project");

    // 2. Start Impact Analysis Child Span
    let impact_span_id = collector.start_span(
        "impact_001",
        Some(&root_span_id),
        "onaji.impact.analysis",
        SpanKind::ImpactAnalysis,
    );
    collector.set_attribute(&impact_span_id, "resource.changed", "openapi_spec");
    collector.set_attribute(&impact_span_id, "affected_tasks.count", "2");
    collector.finish_span(&impact_span_id, SpanStatus::Ok);

    // 3. Task 1 (Cache MISS -> Success)
    let task1_span_id = collector.start_span(
        "task_validate_openapi",
        Some(&root_span_id),
        "onaji.task.execute",
        SpanKind::TaskExecute,
    );
    collector.set_attribute(&task1_span_id, "task.id", "validate_openapi");
    collector.set_attribute(&task1_span_id, "task.action", "redocly lint openapi.yaml");
    collector.set_attribute(&task1_span_id, "causal.trigger", "openapi_spec");
    collector.set_attribute(&task1_span_id, "cache.tier", "local");
    collector.set_attribute(&task1_span_id, "cache.key", "sha256:abc123def456");

    // Cache Lookup Child Span
    let cache_span_id = collector.start_span(
        "cache_lookup_001",
        Some(&task1_span_id),
        "onaji.cache.lookup",
        SpanKind::CacheLookup,
    );
    collector.set_attribute(&cache_span_id, "cache.hit", "false");
    collector.finish_span(&cache_span_id, SpanStatus::Ok);

    collector.add_event(
        &task1_span_id,
        SpanEvent::new("process.spawned").with_attr("pid", "40123"),
    );
    collector.finish_span(&task1_span_id, SpanStatus::Ok);
    collector.record_metric(false, true, false);

    // 4. Task 2 (Cache HIT -> Cached)
    let task2_span_id = collector.start_span(
        "task_generate_kotlin",
        Some(&root_span_id),
        "onaji.task.execute",
        SpanKind::TaskExecute,
    );
    collector.set_attribute(&task2_span_id, "task.id", "generate_kotlin");
    collector.set_attribute(&task2_span_id, "causal.trigger", "openapi_spec");
    collector.set_attribute(&task2_span_id, "cache.tier", "remote");
    collector.set_attribute(&task2_span_id, "cache.key", "sha256:fed987cba654");
    collector.finish_span(&task2_span_id, SpanStatus::Cached);
    collector.record_metric(true, true, false);

    // Complete Root Span
    collector.finish_span(&root_span_id, SpanStatus::Ok);

    // -----------------------------------------------------------------------
    // Assertions
    // -----------------------------------------------------------------------
    let spans = collector.get_spans();
    assert_eq!(spans.len(), 5);

    let root = spans.iter().find(|s| s.id == "pipeline_001").unwrap();
    assert_eq!(root.status, SpanStatus::Ok);
    assert_eq!(
        root.attributes.get("causal.trigger").unwrap(),
        "openapi_spec"
    );

    let task1 = spans
        .iter()
        .find(|s| s.id == "task_validate_openapi")
        .unwrap();
    assert_eq!(task1.parent_id.as_deref(), Some("pipeline_001"));
    assert_eq!(task1.events.len(), 1);
    assert_eq!(task1.events[0].name, "process.spawned");

    let task2 = spans
        .iter()
        .find(|s| s.id == "task_generate_kotlin")
        .unwrap();
    assert_eq!(task2.status, SpanStatus::Cached);

    // Metrics verification
    let metrics = collector.get_metrics();
    assert_eq!(metrics.tasks_total, 2);
    assert_eq!(metrics.tasks_cached, 1);
    assert_eq!(metrics.tasks_success, 1);
    assert_eq!(metrics.cache_hit_ratio(), 0.5);

    // Causal Explanation verification
    let explanation = collector.explain_task("validate_openapi");
    assert!(explanation.contains("Task: validate_openapi"));
    assert!(explanation.contains("Triggered by Resource: openapi_spec"));
    assert!(explanation.contains("Cache Key: sha256:abc123def456"));

    // JSON Export format validation
    let json_trace = collector.to_json();
    assert!(json_trace.contains("onaji.pipeline.run"));
    assert!(json_trace.contains("onaji.impact.analysis"));
}

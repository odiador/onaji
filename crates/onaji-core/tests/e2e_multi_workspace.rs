use onaji_core::config::ConfigLoader;
use onaji_core::domain::{ArtifactId, ResourceId, TaskId};
use onaji_core::workspace::{CrossWorkspaceDep, MultiWorkspaceGraph, Workspace, WorkspaceId};

#[test]
fn test_multi_workspace_monorepo_isolation_and_cross_impact() {
    let mut multi_graph = MultiWorkspaceGraph::new();

    // 1. Backend Workspace (Rust/Axum producing OpenAPI contract artifact)
    let backend_yaml = r#"
version: "1"
name: "backend"
resources:
  openapi_spec:
    type: file
    path: "./api/openapi.yaml"
artifacts:
  openapi_contract:
    description: "Exported OpenAPI Specification"
tasks:
  export_openapi:
    inputs: [openapi_spec]
    outputs: [openapi_contract]
    run: "cargo run --bin export-openapi"
"#;
    let (backend_manifest, backend_dag) = ConfigLoader::load_from_str(backend_yaml).unwrap();
    let backend_ws = Workspace {
        id: WorkspaceId::new("backend"),
        root_dir: std::path::PathBuf::from("/monorepo/backend"),
        manifest_path: std::path::PathBuf::from("/monorepo/backend/onaji.yaml"),
        manifest: backend_manifest,
        dag: backend_dag,
        dependencies: Vec::new(),
    };
    multi_graph.add_workspace(backend_ws).unwrap();

    // 2. Mobile Workspace (Kotlin/Android consuming backend:openapi_contract)
    let mobile_yaml = r#"
version: "1"
name: "mobile"
artifacts:
  openapi_contract:
    description: "Imported from backend"
  kotlin_client:
    description: "Generated Kotlin Network Client"
  android_apk:
    description: "Built Android Debug APK"
tasks:
  generate_kotlin:
    inputs: [openapi_contract]
    outputs: [kotlin_client]
    run: "openapi-generator generate"
  compile_android:
    inputs: [kotlin_client]
    outputs: [android_apk]
    run: "./gradlew assembleDebug"
"#;
    let (mobile_manifest, mobile_dag) = ConfigLoader::load_from_str(mobile_yaml).unwrap();
    let mobile_ws = Workspace {
        id: WorkspaceId::new("mobile"),
        root_dir: std::path::PathBuf::from("/monorepo/mobile"),
        manifest_path: std::path::PathBuf::from("/monorepo/mobile/onaji.yaml"),
        manifest: mobile_manifest,
        dag: mobile_dag,
        dependencies: vec![CrossWorkspaceDep {
            workspace: WorkspaceId::new("backend"),
            artifact: ArtifactId::new("openapi_contract"),
        }],
    };
    multi_graph.add_workspace(mobile_ws).unwrap();

    // 3. Web Workspace (Independent Next.js application)
    let web_yaml = r#"
version: "1"
name: "web"
resources:
  web_src:
    type: file
    path: "./src/index.tsx"
artifacts:
  web_bundle:
    description: "Compiled Next.js production bundle"
tasks:
  build_web:
    inputs: [web_src]
    outputs: [web_bundle]
    run: "npm run build"
"#;
    let (web_manifest, web_dag) = ConfigLoader::load_from_str(web_yaml).unwrap();
    let web_ws = Workspace {
        id: WorkspaceId::new("web"),
        root_dir: std::path::PathBuf::from("/monorepo/web"),
        manifest_path: std::path::PathBuf::from("/monorepo/web/onaji.yaml"),
        manifest: web_manifest,
        dag: web_dag,
        dependencies: Vec::new(),
    };
    multi_graph.add_workspace(web_ws).unwrap();

    // Connect cross-workspace edges and validate acyclic meta-graph
    multi_graph.connect_dependencies().unwrap();

    // -----------------------------------------------------------------------
    // Scenario A: Workspace Isolation (Web modification does not leak)
    // -----------------------------------------------------------------------
    let web_impact = multi_graph
        .analyze_impact(&WorkspaceId::new("web"), &ResourceId::new("web_src"))
        .expect("Web impact calculated");

    assert_eq!(web_impact.root_workspace.as_str(), "web");
    assert_eq!(
        web_impact.local_affected_tasks,
        vec![TaskId::new("build_web")]
    );
    assert!(web_impact.cross_workspace_impacts.is_empty()); // 0 cross-workspace leakage

    // -----------------------------------------------------------------------
    // Scenario B: Cross-Workspace Causal Propagation (Backend -> Mobile)
    // -----------------------------------------------------------------------
    let backend_impact = multi_graph
        .analyze_impact(
            &WorkspaceId::new("backend"),
            &ResourceId::new("openapi_spec"),
        )
        .expect("Backend impact calculated");

    assert_eq!(backend_impact.root_workspace.as_str(), "backend");
    assert_eq!(
        backend_impact.local_affected_tasks,
        vec![TaskId::new("export_openapi")]
    );

    assert_eq!(backend_impact.cross_workspace_impacts.len(), 1);
    let mobile_cross = &backend_impact.cross_workspace_impacts[0];
    assert_eq!(mobile_cross.workspace_id.as_str(), "mobile");
    assert_eq!(
        mobile_cross.affected_tasks,
        vec![TaskId::new("generate_kotlin")]
    );
}

#[test]
fn test_multi_workspace_cycle_rejection() {
    let mut cyclic_graph = MultiWorkspaceGraph::new();

    let yaml_a = r#"
version: "1"
name: "ws_a"
artifacts:
  art_a: {}
  art_b: {}
tasks:
  task_a:
    inputs: [art_b]
    outputs: [art_a]
    run: "echo a"
"#;
    let (manifest_a, dag_a) = ConfigLoader::load_from_str(yaml_a).unwrap();
    let ws_a = Workspace {
        id: WorkspaceId::new("ws_a"),
        root_dir: std::path::PathBuf::from("/monorepo/ws_a"),
        manifest_path: std::path::PathBuf::from("/monorepo/ws_a/onaji.yaml"),
        manifest: manifest_a,
        dag: dag_a,
        dependencies: vec![CrossWorkspaceDep {
            workspace: WorkspaceId::new("ws_b"),
            artifact: ArtifactId::new("art_b"),
        }],
    };
    cyclic_graph.add_workspace(ws_a).unwrap();

    let yaml_b = r#"
version: "1"
name: "ws_b"
artifacts:
  art_a: {}
  art_b: {}
tasks:
  task_b:
    inputs: [art_a]
    outputs: [art_b]
    run: "echo b"
"#;
    let (manifest_b, dag_b) = ConfigLoader::load_from_str(yaml_b).unwrap();
    let ws_b = Workspace {
        id: WorkspaceId::new("ws_b"),
        root_dir: std::path::PathBuf::from("/monorepo/ws_b"),
        manifest_path: std::path::PathBuf::from("/monorepo/ws_b/onaji.yaml"),
        manifest: manifest_b,
        dag: dag_b,
        dependencies: vec![CrossWorkspaceDep {
            workspace: WorkspaceId::new("ws_a"),
            artifact: ArtifactId::new("art_a"),
        }],
    };
    cyclic_graph.add_workspace(ws_b).unwrap();

    // Must reject cross-workspace cyclic dependency!
    let res = cyclic_graph.connect_dependencies();
    assert!(res.is_err());
}

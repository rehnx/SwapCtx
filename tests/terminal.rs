//! Output capture must never activate terminal styling or a provider session.
use std::process::Command;

#[test]
fn redirected_help_errors_and_status_do_not_emit_ansi() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("isolated data");
    let run = |args: &[&str], no_color: Option<&str>| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_swapctx"));
        command
            .current_dir(root.path())
            .arg("--data-dir")
            .arg(&data)
            .args(args);
        command.env_remove("NO_COLOR");
        // Forced color from a parent process must not leak escapes into a log.
        command.env("CLICOLOR_FORCE", "1").env("FORCE_COLOR", "1");
        if let Some(value) = no_color {
            command.env("NO_COLOR", value);
        }
        command.output().unwrap()
    };
    for no_color in [None, Some(""), Some("1")] {
        for (args, expected) in [
            (&["--help"][..], 0),
            (&["--invalid-option"][..], 2),
            (&[][..], 1),
        ] {
            let output = run(args, no_color);
            assert_eq!(output.status.code(), Some(expected));
            assert!(!output.stdout.contains(&0x1b));
            assert!(!output.stderr.contains(&0x1b));
            assert!(
                !data.exists(),
                "informational/error commands must not create a store"
            );
        }
    }
    assert!(run(&["init"], None).status.success());
    for no_color in [None, Some(""), Some("1")] {
        let output = run(&["status"], no_color);
        assert!(output.status.success());
        assert!(!output.stdout.contains(&0x1b));
    }
}

// Offline shell-dispatch fixture, never a Codex executable.
// Models rust-v0.163.0 hooks/engine/command_runner.rs: /C uses raw_arg
// with an outer quote pair; other configured shells use an ordinary arg.
// cmd-escaped additionally reproduces historical issue #32402.
#[cfg(windows)]
fn main() {
    use std::os::windows::process::CommandExt;
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().unwrap();
    let shell = args.next().unwrap();
    let line = args.next().unwrap();
    assert!(args.next().is_none());
    let mut command = std::process::Command::new(shell);
    match mode.to_str().unwrap() {
        // /D disables machine/user cmd AutoRun for deterministic fixtures.
        "cmd-raw" => {
            command
                .args(["/D", "/C"])
                .raw_arg(format!("\"{}\"", line.to_str().unwrap()));
        }
        "cmd-escaped" => {
            command.args(["/D", "/C"]).arg(line);
        }
        "powershell" => {
            command
                .args(["-NoProfile", "-NonInteractive", "-Command"])
                .arg(line);
        }
        _ => panic!("invalid fixture mode"),
    }
    let status = command.status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
#[cfg(not(windows))]
fn main() {
    panic!("Windows-only fixture");
}

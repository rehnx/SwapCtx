// Offline console harness. Creates a private console, feeds q or selects the
// synthetic provider without context, and records its screen. No user console.
#[cfg(windows)]
fn main() -> std::io::Result<()> {
    use std::{
        ffi::c_void,
        fs::OpenOptions,
        os::windows::io::AsRawHandle,
        process::{Command, Stdio},
    };
    #[repr(C)]
    struct Coord {
        x: i16,
        y: i16,
    }
    #[repr(C)]
    struct InputRecord {
        kind: u16,
        padding: u16,
        key_down: i32,
        repeat: u16,
        key: u16,
        scan: u16,
        ch: u16,
        control: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FreeConsole() -> i32;
        fn AllocConsole() -> i32;
        fn SetConsoleMode(handle: *mut c_void, mode: u32) -> i32;
        fn WriteConsoleInputW(
            handle: *mut c_void,
            records: *const InputRecord,
            count: u32,
            written: *mut u32,
        ) -> i32;
        fn ReadConsoleOutputCharacterW(
            handle: *mut c_void,
            text: *mut u16,
            count: u32,
            start: Coord,
            read: *mut u32,
        ) -> i32;
    }
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    assert_eq!(args.len(), 4, "shell, script, report, mode");
    // SAFETY: this process owns its private console and all file handles below.
    unsafe {
        FreeConsole();
    }
    if unsafe { AllocConsole() } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let input = OpenOptions::new().read(true).write(true).open("CONIN$")?;
    let output = OpenOptions::new().read(true).write(true).open("CONOUT$")?;
    // Begin with VT disabled to exercise actual enablement, and enable line I/O.
    if unsafe { SetConsoleMode(output.as_raw_handle(), 3) } == 0
        || unsafe { SetConsoleMode(input.as_raw_handle(), 7) } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    let mode = args[3].to_string_lossy();
    let mut command = Command::new(&args[0]);
    if args[0]
        .to_string_lossy()
        .to_ascii_lowercase()
        .ends_with("cmd.exe")
    {
        command.args(["/d", "/v:off", "/c"]).arg(&args[1]);
    } else {
        command
            .args([
                "-NoLogo",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&args[1]);
    }
    command
        .stdin(Stdio::from(input.try_clone()?))
        .stdout(Stdio::from(output.try_clone()?))
        .stderr(Stdio::from(output.try_clone()?))
        .env_remove("WT_SESSION")
        .env_remove("TERM")
        .env_remove("NO_COLOR");
    if mode == "no-color" || mode == "no-color-empty" {
        command
            .env("NO_COLOR", if mode == "no-color" { "1" } else { "" })
            .env("WT_SESSION", "synthetic-capability-test");
    }
    if mode == "dumb" {
        command
            .env("TERM", "dumb")
            .env("WT_SESSION", "synthetic-capability-test");
    }
    if mode == "styled" {
        command.env("WT_SESSION", "synthetic-capability-test");
    }
    let mut child = command.spawn()?;
    let keys: &[u8] = if mode == "handoff" { b"1\rn\r" } else { b"q\r" };
    let records = keys
        .iter()
        .map(|ch| InputRecord {
            kind: 1,
            padding: 0,
            key_down: 1,
            repeat: 1,
            key: ch.to_ascii_uppercase() as u16,
            scan: 0,
            ch: *ch as u16,
            control: 0,
        })
        .collect::<Vec<_>>();
    let mut sent_input = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "menu did not accept q in a real console",
            ));
        }
        // Wait for the actual prompt so shell startup cannot consume our input.
        if !sent_input {
            let mut text = vec![0_u16; 8000];
            let mut read = 0;
            if unsafe {
                ReadConsoleOutputCharacterW(
                    output.as_raw_handle(),
                    text.as_mut_ptr(),
                    text.len() as u32,
                    Coord { x: 0, y: 0 },
                    &mut read,
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            let text = String::from_utf16_lossy(&text[..read as usize]);
            if text.contains("Select:") || text.contains("Select an option") {
                let mut written = 0;
                if unsafe {
                    WriteConsoleInputW(
                        input.as_raw_handle(),
                        records.as_ptr(),
                        records.len() as u32,
                        &mut written,
                    )
                } == 0
                    || written != records.len() as u32
                {
                    return Err(std::io::Error::last_os_error());
                }
                sent_input = true;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(
        status.code(),
        Some(if mode == "handoff" { 37 } else { 0 }),
        "menu shell exit: {status}"
    );
    assert!(sent_input, "menu must display a prompt before quitting");
    let mut screen = vec![0_u16; 8000];
    let mut read = 0;
    if unsafe {
        ReadConsoleOutputCharacterW(
            output.as_raw_handle(),
            screen.as_mut_ptr(),
            screen.len() as u32,
            Coord { x: 0, y: 0 },
            &mut read,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    std::fs::write(&args[2], String::from_utf16_lossy(&screen[..read as usize]))?;
    unsafe {
        FreeConsole();
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    panic!("Windows console fixture requires Windows");
}

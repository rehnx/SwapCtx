// Offline synthetic executable only. Delegates to our fixture, never a provider.
fn main() {
    let status = std::process::Command::new(std::env::var_os("SWAPCTX_FAKE_NODE").unwrap())
        .arg(std::env::var_os("SWAPCTX_FAKE_SCRIPT").unwrap())
        .args(std::env::args_os().skip(1))
        .status()
        .unwrap();
    std::process::exit(status.code().unwrap_or(1));
}

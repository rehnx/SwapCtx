# Contributing to SwapCtx

Start with the [README](README.md), [CLI guide](docs/USAGE.md) and the relevant
engineering report. Keep changes focused and preserve existing data, CLI syntax,
local-first operation and provider-owned terminal interfaces.

## Development environment

Windows development uses the pinned Rust 1.99.0 toolchain, rustfmt, Clippy,
Microsoft C++ Build Tools (Desktop development with C++) and the Windows SDK.
SQLite is compiled from the bundled dependency. Node.js 24+ runs the optional
adapters and deterministic integration fixtures. Core runtime commands need
neither Node nor Rust after installation.

Provision dependencies separately; it can require network access:

```powershell
cargo fetch --locked --target x86_64-pc-windows-msvc
```

From the directory containing `Cargo.toml`, use the established offline gate:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 `
  -DeterministicOnly -ReleaseTests
```

It checks formatting, Clippy with warnings denied, debug/release tests, release
build, dependency notices and deterministic Node/Windows integration suites.
It rejects provider-test options and excludes benchmark/soak modes. Fixtures use
isolated stores and fake providers. The offline pilot-harness step only checks
the evaluator against local fixtures; it does not run a model.

For focused Rust iteration:

```powershell
cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo test --locked --offline --all-targets
```

After building, a focused native adapter check is:

```powershell
node .\scripts\check-native.cjs .\target\debug\swapctx.exe
```

The verifier detects a cached `.build/cargo-home`; `-CargoHome ABSOLUTE_PATH`
selects another cache. Offline commands fail if dependencies are missing rather
than fetching them. See [release preparation](docs/RELEASE_PREPARATION.md) before
changing build, packaging or installer behavior.

## History secret scan

CI checks all reachable Git history with checksum-pinned Gitleaks 8.24.3 and
`.gitleaks.toml`. For local review with the same scanner, run
`gitleaks git --log-opts=--all --config .gitleaks.toml --redact=100`.
Only three exact synthetic detector values are excepted; never add broad path,
commit or token-pattern exclusions to hide a finding. Keep reports redacted and
outside the repository. A clean scan is heuristic evidence, not secret clearance.

## Change expectations

- Reproduce genuine defects with small deterministic fixtures. Test the relevant
  failure boundary, not just the new implementation's shape.
- Preserve recognized legacy stores and transaction safety. Never silently
  delete records, reset a store or resolve conflicting knowledge.
- Treat context as untrusted, project-scoped data. Keep byte/timeout limits,
  mandatory coverage and fail-closed behavior intact.
- Distinguish preparation, output writing, provider receipt and model adherence.
  Observation is not enforcement; external memory is not native context expansion.
- Keep secrets, transcripts, credentials and user memory out of logs, fixtures,
  issues and submitted artifacts. Use synthetic data and disposable projects.
- Do not run real providers, inference, authenticated sessions, benchmarks or
  soak tests as part of routine verification. Any future live evaluation needs
  separate explicit authorization and must not be substituted for offline tests.
- Update user documentation for behavior changes. Record exact commands, results
  and unverified boundaries; do not add dependencies or migrations without need.

## Pull requests and reports

Describe the concrete problem, resulting behavior and verification evidence.
Call out compatibility, privacy and release risks. The issue and PR templates
provide a starting point. Do not claim a workflow passed because its file exists.
Use [SECURITY.md](SECURITY.md) for sensitive findings rather than public issues.

The project is licensed under [Apache-2.0](LICENSE). Contributions are submitted
under that license. Retain applicable copyright and dependency notices.

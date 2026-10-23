# Contributing to SwapCtx

SwapCtx welcomes focused fixes, documentation, synthetic test cases, and workflow
proposals. Start with the [README](README.md), [documentation index](docs/README.md),
and [security policy](SECURITY.md). Follow the [community conduct guidelines](CODE_OF_CONDUCT.md).

## Your first contribution

1. Check [open issues](https://github.com/rehnx/SwapCtx/issues) and existing pull
   requests for related work. A small documentation fix can go straight to a PR;
   discuss substantial behavior or contract changes in an issue first.
2. Fork the repository if you do not have write access, clone your fork, and create
   a development branch. Keep the change focused on one problem.
3. Make the change, run the appropriate checks below, and open a PR against `main`.
   Explain the problem, resulting behavior, checks, and any unverified platforms.

Useful first contributions include clearer installation instructions,
troubleshooting with synthetic reproductions, and tests for scoped retrieval or
fake-provider events. No live AI subscription is needed to contribute.

## Development setup

On Windows, install Rust through rustup and Microsoft C++ Build Tools with the
**Desktop development with C++** workload and Windows SDK. Bundled SQLite needs
that C compiler. Use the pinned toolchain in `rust-toolchain.toml` (Rust 1.99.0).
Node.js **24+** runs the deterministic adapter and documentation checks.

```powershell
# Replace YOUR-LOGIN with your GitHub login if contributing through a fork.
git clone https://github.com/YOUR-LOGIN/SwapCtx.git
cd SwapCtx
git switch -c docs/clarify-installation

# Provision the pinned toolchain and locked Windows dependencies.
rustup toolchain install 1.99.0 --profile minimal --component rustfmt --component clippy
cargo fetch --locked --target x86_64-pc-windows-msvc
cargo build --locked --release
.\target\release\swapctx.exe --version
```

Toolchain and dependency provisioning may use the network. Routine verification
uses disposable stores, synthetic data, and fake providers. Source builds are
for development; ordinary `cargo build` output may need the VC runtime, unlike
the published static-runtime portable package.

## Verification

For a documentation-only change, use an existing trusted binary built from the
applicable product source; no release rebuild is needed:

```powershell
node .\scripts\check-docs.cjs .\target\release\swapctx.exe
```

The documentation check validates local links/anchors, PowerShell syntax, a small
Mermaid grammar, and safe README commands in disposable stores. Provider examples
use parser help only. It does not fetch external links or render GitHub pages.
Check new pages and external links as well; the script has an explicit file list.

For implementation changes, run the complete Windows deterministic gate after
provisioning dependencies:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 `
  -DeterministicOnly -ReleaseTests
```

This covers formatting, Clippy, debug/release tests, the release build, notices,
and the deterministic Node suites in `.github/workflows/ci.yml`. Its Cargo stages
are equivalent to:

```powershell
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked --release
cargo test --locked --release --all-targets
```

The verifier selects a cached `.build/cargo-home` when present;
`-CargoHome ABSOLUTE_PATH` selects another cache. Its offline mode fails if
locked dependencies are missing. Linux contributors can run the Cargo commands
and portable Node suites from CI against `target/release/swapctx`. Report
Windows-only skips explicitly: Linux results do not establish Windows behavior.

Changes affecting release production additionally need the **fresh Windows
producer** in [release preparation](docs/RELEASE_PREPARATION.md), using a new
candidate destination. Never overwrite existing candidates. Documentation changes
after publication do not retag or rebuild `v0.1.0-preview`.

After `Cargo.lock` changes, provision locked Windows dependencies separately,
then run `node scripts/third-party.cjs --write` and review the generated notices.
Do not edit generated inventory by hand. The published tag and assets remain fixed.

Do not start installed providers, authenticated sessions, inference, model
benchmarks, or soak runs as routine verification. They require separate explicit
authorization. Offline pilot/soak evaluator fixtures make no model requests.

## Repository map

| Location | Responsibility |
| --- | --- |
| `src/` | Rust CLI, SQLite storage, memory lifecycle, retrieval, policies, handoffs, and recovery |
| `integrations/native/` | Shared experimental Codex/Claude lifecycle adapter |
| `integrations/codex/` | Separate managed Codex console and context adapters |
| `tests/` | Rust integration tests and synthetic provider/store fixtures |
| `scripts/` | Offline checks, Windows build/package tools, and recipient acceptance |
| `docs/` | User guides, protocol contracts, engineering reports, and scoped evidence |

See [architecture and contracts](docs/README.md#architecture-and-contracts) for
retrieval and storage behavior, and [unified integration](docs/UNIFIED_INTEGRATION.md)
for historical implementation decisions.

## Engineering boundaries

- Keep storage/retrieval local-first: no network client, telemetry, background
  collection, or automatic transcript ingestion. Stores and exports are plaintext.
- Preserve CLI/JSON contracts, recognized legacy stores, project isolation, and
  transactional recovery. Never reset unknown or corrupt data as repair, silently
  merge conflicting knowledge, or overwrite backups and exports.
- Provider sharing is opt-in and scoped. Retrieve fresh validated context, fail
  closed, preserve normal trust, and never modify provider settings, `AGENTS.md`,
  or `CLAUDE.md` as integration setup.
- Distinguish local preparation/output from provider receipt and model adherence.
  Boundary rules are advisory; hashes are not signatures.

Keep fixtures synthetic. Never commit tokens, credentials, personal memory,
provider configurations, or transcripts. Review diagnostic reports before sharing.
CI scans reachable Git history with Gitleaks 8.24.3 and `.gitleaks.toml`. If installed:

```powershell
gitleaks git --log-opts=--all --config .gitleaks.toml --redact=100
```

Only three exact synthetic detector values are excepted. Do not add broad path,
commit, or token-pattern exclusions. Keep reports redacted and outside the repo.
A clean heuristic scan does not prove absence of secrets.

## Submit a pull request

Use the [PR template](.github/pull_request_template.md) to describe the problem,
result, verification, and compatibility risks. Include exact commands and skipped
checks. Keep historical evidence tied to its original source/environment; use a
new record for new evidence. Let applicable CI complete before merging.

Contributions are submitted under [Apache-2.0](LICENSE). For non-sensitive help,
see [SUPPORT.md](SUPPORT.md). Report vulnerabilities privately through
[SECURITY.md](SECURITY.md), never in a public PR or issue.

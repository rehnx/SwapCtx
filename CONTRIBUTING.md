# Contributing to SwapCtx

Welcome to SwapCtx! We are thrilled that you want to contribute. SwapCtx is a Windows-first, privacy-first Rust CLI for persistent AI coding-agent project memory and reliability.

Before starting, please review the [README](README.md), [CLI guide](docs/USAGE.md), and [SECURITY.md](SECURITY.md). Keep changes focused, deterministic, and privacy-preserving, respecting local-first operation and provider-owned terminal interfaces.

---

## Quick Start for Contributors

Follow these steps to set up your environment, build SwapCtx, and submit your first contribution.

### 1. Clone the Repository & Create a Branch

```powershell
git clone https://github.com/rehnx/SwapCtx.git
cd SwapCtx
# Create a feature or dev branch (e.g. jules/docs-improvement or fix/memory-search-bug)
git checkout -b jules/docs-improvement
```

### 2. Set Up Toolchain & Fetch Dependencies

SwapCtx relies on the pinned Rust toolchain in `rust-toolchain.toml` (Rust 1.99.0) and C++ Build Tools (with Windows SDK) for bundled SQLite compilation. Node.js 24+ is used for adapter integration fixtures and script verification.

```powershell
# Fetch Rust dependencies
cargo fetch --locked --target x86_64-pc-windows-msvc
```

### 3. Build & Run Tests Locally

```powershell
# Check formatting
cargo fmt --check

# Run Clippy lints with warnings denied
cargo clippy --locked --offline --all-targets -- -D warnings

# Run all Rust unit and integration tests
cargo test --locked --offline --all-targets

# Build release binary locally
cargo build --locked --release
```

### 4. Run Verification Suite

For full Windows verification before submitting a PR, execute the deterministic verification script:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 `
  -DeterministicOnly -ReleaseTests
```

This checks formatting, Clippy, debug/release tests, release builds, dependency notices, and deterministic Node integration suites using fake providers and isolated test databases.

---

## Repository Architecture & Map

Understanding where different components reside helps keep contributions clean and modular:

| Component / Folder | Responsibility | Tech Stack / Tooling |
| --- | --- | --- |
| `src/` | **Core Engine & CLI**: SQLite storage, project registration, retrieval ranking, policy enforcement, backup/restore, export/import, and CLI parser commands. | Rust 1.99.0, SQLite (bundled), Clap |
| `integrations/` | **Provider Lifecycle Adapters**: Context hook bridge for Codex and Claude Code. Connects provider lifecycle hooks to SwapCtx `context` JSON protocol. | Node.js 24+ (TypeScript/JS) |
| `docs/` | **Technical Specifications & Engineering Reports**: In-depth docs on protocol contracts, release engineering, security hardening, and roadmap. | Markdown |
| `scripts/` | **CI, Verification & Release Tools**: Verification runners (`verify.ps1`), documentation validators (`check-docs.cjs`), third-party license inventory (`third-party.cjs`), and native adapter checkers (`check-native.cjs`). | PowerShell, Node.js |
| `tests/` | **Deterministic Integration Tests**: End-to-end integration tests using fake providers, temporary directories, and synthetic records. | Rust & Node test runners |

---

## Contribution Guidelines & Workflow

### First-Contribution Ideas
If you're looking for good places to start contributing:
- **Documentation & Examples**: Clarifying command outputs, improving troubleshooting scenarios in [SUPPORT.md](SUPPORT.md), or adding synthetic context protocol examples in `docs/`.
- **Test Coverage**: Adding deterministic unit tests or fake provider fixture cases in `tests/`.
- **CLI Diagnostics**: Improving error messages or diagnostic explanations in `swapctx doctor` and `swapctx status`.

### Key Engineering Boundaries
- **No Telemetry or Remote Core**: Core storage/retrieval must remain strictly local-first with zero background network collection or remote telemetry.
- **Fail Closed & Immutable History**: Never silently overwrite, merge, or delete user memories, backups, or conflicting records. Treat records as immutable with explicit supersession or invalidation reasons.
- **Provider Isolation**: Native integration adapters must never alter provider settings (`~/.codex/config.json`, etc.), `AGENTS.md`, `CLAUDE.md`, or bypass user permission prompts.
- **Preparation vs. Adherence**: Do not claim that successful local context output guarantees model adherence or receipt by external providers.

### Local Secret Scanning
CI checks reachable Git history with Gitleaks 8.24.3 and `.gitleaks.toml`. Run a local check before pushing:
```powershell
gitleaks git --log-opts=--all --config .gitleaks.toml --redact=100
```
Keep test fixtures strictly synthetic; never commit real tokens, keys, passwords, or personal project data.

---

## Submitting Pull Requests

1. **Keep Changes Focused**: Address one problem or enhancement per PR.
2. **Follow PR Templates**: Fill out `.github/pull_request_template.md` with problem description, offline verification commands, and risk assessments.
3. **Verify Documentation Links**: If editing documentation, run `node .\scripts\check-docs.cjs` to confirm no broken internal links exist.
4. **License & Copyright**: SwapCtx is released under the [Apache-2.0 License](LICENSE). All contributions are submitted under this license.

Need help or troubleshooting? Check [SUPPORT.md](SUPPORT.md) or open a public issue. Security vulnerabilities must follow [SECURITY.md](SECURITY.md).

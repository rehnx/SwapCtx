# Working on SwapCtx

Read README.md, CONTRIBUTING.md and SECURITY.md before implementation. This is a
Windows-first Rust CLI with local SQLite memory and optional Node 24+ adapters.
Use the pinned toolchain in rust-toolchain.toml. Preserve CLI and JSON contracts,
recognized legacy stores, project isolation and transactional recovery.

## Verification

- `cargo fmt --check`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo test --locked --all-targets`
- `cargo build --locked --release`
- `cargo test --locked --release --all-targets`
- Run the deterministic Node suites in `.github/workflows/ci.yml` against that binary.
- On Windows, use `scripts/verify.ps1 -DeterministicOnly -ReleaseTests` and the
  fresh producer described in docs/RELEASE_PREPARATION.md. Linux results do not
  establish Windows behavior. Report Windows-only skips explicitly.
- After Cargo.lock changes, provision locked Windows dependencies separately,
  then regenerate and review notices with `node scripts/third-party.cjs --write`.

Routine tests use disposable stores and fake providers. Do not start installed
providers, authenticated sessions, inference, model benchmarks or soak runs without
explicit user authorization. Offline evaluator fixtures make no model requests.

## Boundaries

- No telemetry, background collection, automatic transcript ingestion or network
  client in the storage/retrieval core. Stores and exports remain plaintext.
- Provider sharing is opt-in; callbacks retrieve fresh scoped context and fail
  closed. Never reuse stale context, bypass trust, or modify provider settings,
  AGENTS.md or CLAUDE.md as an integration setup mechanism.
- Local preparation/output is not provider receipt or model adherence. Boundary
  rules do not enforce filesystem permissions. Hashes are not signatures.
- Preserve user files and unknown stores. Never reset data as repair or overwrite
  backups, exports or release candidates. Do not force-push unrelated history.
- Work on a development branch. Respect the current user's authorization for
  pushes/PRs/CI; publishing, mainline merges and visibility changes are separate.

Current integration decisions, evidence and next steps: docs/UNIFIED_INTEGRATION.md.

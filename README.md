# SwapCtx

Local-first external context records and explicit project path-rule checks for AI coding projects. Rust CLI, SQLite storage, Windows first.

SwapCtx stores requirements, decisions, notes, and path rules that you deliberately save. It does **not** expand a model's native context window, inspect hidden model state, connect to agents, or automatically prevent changes. A successful path check is a snapshot observation, not authorization or enforcement.

## Foundation status

The supplied v0.1 implementation has been audited and hardened without adding agent integrations. See [the foundation audit](docs/FOUNDATION_AUDIT.md) for findings, behavior changes, verification evidence, and remaining work.

**Release verification is incomplete:** the development environment has no Rust toolchain and could not download one. Supplemental SQL checks passed; the Rust test suite, compiler, formatter, Clippy, and executable have not been run. The included CI workflow must pass before treating this revision as a verified release.

## Build and verify on Windows

Install Rust and the Microsoft C++ Build Tools with the **Desktop development with C++** workload. Use an x64 native tools terminal if the linker/build tools are not discovered automatically. The bundled SQLite dependency needs a working C compiler.

From the folder containing `Cargo.toml`:

```powershell
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

The executable is `target\release\swapctx.exe`. The first build downloads dependencies; the executable has no network operations. After the first successful dependency resolution, commit the generated `Cargo.lock` and use `--locked` for tests and release builds. No lockfile has been fabricated for this unbuilt source package.

`.github/workflows/ci.yml` contains Windows and Linux checks. It has not been executed. Put the `SwapCtx` source folder at repository root when publishing it so GitHub discovers that workflow. CI currently formats its build inputs; once local Rust verification is available, commit rustfmt's output and change that step to `cargo fmt --check`.

For supplemental SQL verification with Node.js 24 (optional, not a runtime dependency):

```powershell
node scripts/check-sql.cjs
```

This executes SQL extracted from the Rust sources using Node's SQLite library. It **does not** run the Rust application, validate its Rust bindings, or replace `cargo test`.

## Use

```powershell
$SwapCtx = 'C:\path\to\SwapCtx\target\release\swapctx.exe'
Set-Location 'C:\path\to\your-project'

& $SwapCtx init
& $SwapCtx memory add --kind requirement 'Preserve authentication behavior'
& $SwapCtx memory add --kind decision 'Use the existing component architecture'
& $SwapCtx memory search 'authentication'
& $SwapCtx memory recent --limit 10

& $SwapCtx protect add --path 'src/auth/**' --reason 'Authentication changes need review'
& $SwapCtx protect list
& $SwapCtx protect check 'src/auth/login.ts'
# Exit code 2 and a PROTECTED diagnostic indicate a match.

& $SwapCtx status
& $SwapCtx privacy
& $SwapCtx --help
```

`init` registers exactly the current folder. Other commands find the nearest registered ancestor, so they work from subdirectories. A separately initialized nested project has separate records and rules; outer rules are not inherited. Relative check targets are relative to the calling directory. Rules remain relative to the selected project root.

An optional global `--data-dir 'C:\path\to\local-data'` stores `swapctx.db` in an explicit directory, useful for tests or separate stores. Relative data directories resolve from the calling directory, so use an absolute directory when moving between project folders. Avoid using shared/network storage for sensitive local records.

## Rule and search contracts

- Rule paths use forward slashes. `src/*.rs` matches one level; `src/**` matches descendants and the `src` directory itself.
- Windows rule matching is ASCII case insensitive, including on case-sensitive Windows directories (conservative extra matches). Other platforms use case-sensitive matching.
- Globs use `globset` byte semantics: `?` matches a byte, and Unicode character classes/case variants do not provide full Windows filename equivalence. Prefer literal names or `*` for Unicode paths. This is a documented limitation, not a complete Windows access-control policy.
- Checks validate both the supplied in-project name and its resolved destination. Existing symlinks/junctions that resolve outside the project, broken links, inaccessible ancestors, and non-directory ancestors cause errors. Missing files are checked using the deepest existing ancestor.
- Parent traversal, empty targets, control characters, alternate data streams, device names, leading spaces, trailing dots/spaces, and drive-relative or root-relative Windows inputs are rejected. Windows check targets must be literal paths, without wildcard/invalid filename characters. Windows device/verbatim namespace input is unsupported. Ordinary absolute paths inside the project are supported.
- A check does not enumerate descendants for recursive operations, identify all hard-link names, or prevent filesystem changes between checking and writing. Check every affected path; a future adapter must implement its own operation-aware checks.
- Search is literal substring matching against content and kind, with ASCII case-insensitive SQLite `LIKE`. `%`, `_`, and `\` are treated literally. Unicode content is preserved, but search does not perform Unicode case folding, stemming, embeddings, or relevance ranking.
- Search/recent results are newest first, with limits from 1 to 100. Empty queries are rejected. Results stream one record at a time.
- New inputs are bounded in UTF-8 bytes: kind 128, content 1 MiB, rule pattern 4 KiB, reason 16 KiB, query 4 KiB. Windows command-line limits may be smaller. Existing saved text is not rewritten or truncated.
- Terminal controls, newlines in saved values, and directional override/isolate characters are escaped for display. Stored content remains unchanged.
- Adding an identical rule with the same reason reports an error. Different reasons for the same pattern are allowed. Rule removal is scoped to the current project.

Exit codes: `0` means command success; `1` means an operational, validation, or database error; `2` means a protected-path match **or** a command-line usage error from the argument parser. Read stderr to distinguish those cases. Wrappers should stop on any nonzero result, not treat an error as permission to write. No hooks or wrappers are installed by SwapCtx.

## Storage, migration, and recovery

Default Windows database: `%LOCALAPPDATA%\SwapCtx\swapctx.db`. Non-Windows development uses `$XDG_DATA_HOME/SwapCtx` or `$HOME/.local/share/SwapCtx`.

Only `init` creates a database and upgrades its schema. Original unversioned databases remain readable; `init` upgrades recognized schemas transactionally to version 1 while preserving record IDs, content, timestamps, and indexes. Reads use read-only connections. Other writes require an existing recognized database. Unknown, incomplete, foreign, or newer schemas fail with an error; SwapCtx does not replace or reset them. Unexpected triggers/views are rejected. This validates schema structure, not every saved data row.

Ambiguous legacy ASCII case aliases for a Windows project cause an error rather than selecting one record arbitrarily. Those registrations are preserved; there is no automatic project merge/rename recovery command.

Foreign keys are enabled on each connection, and lock contention has an explicit five-second timeout. SQLite's rollback journal and default durability settings are retained. No WAL migration or encryption has been introduced.

For a manual backup, stop all SwapCtx processes and other programs using this database, then copy the entire SwapCtx data directory to a protected backup location. **Do not delete journal, WAL, or SHM files to repair a database.** Copying just the main database while another process is writing is unsafe. There is no verified live-backup, restore, export, or corrupt-database repair command yet.

If errors suggest damage, preserve the whole directory before investigating. `init` is not a recovery tool. Restoring a known-good backup requires closing database users and replacing the complete data directory. Recovery procedures still need fault-injection and restore testing before production guarantees.

## Privacy and capability boundaries

No AI calls, uploads, telemetry, credential discovery, project scanning, or agent history ingestion are implemented. SQLite storage is **not encrypted**, and the application does not configure Windows ACLs. Local account permissions and disk encryption are the current protection. Anyone with access to the database can read or change saved records; it is not a tamper-proof audit log. Avoid saving credentials.

External coding agents retain their own network behavior. Saved requirements only influence an agent if a person or a supported adapter supplies them to that agent. Rule monitoring is not enforcement. No model-internal memory-loss explanation, hidden-state access, native context expansion, or agent API compatibility is claimed.

## Next development work

First finish Rust build/test verification, commit a dependency lockfile, audit dependencies, and test Windows junctions and recovery scenarios. Then prioritize reliable local backup/restore, deliberate memory lifecycle management, and provenance-aware retrieval. Agent adapters and evidence-based debugging should follow documented platform capability checks and operation-level tests, with explicit distinction between observations and actual intercepted actions.

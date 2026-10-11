# SwapCtx

Local-first external context records and manual project boundary checks. Rust CLI, SQLite storage, Windows first.

SwapCtx preserves requirements, decisions, notes, and rules that you explicitly save. It does **not** expand native model context, inspect hidden model state, integrate with agents, or intercept their actions. A check is an observation at a point in time, not enforcement or authorization.

## Foundation status

Native Windows formatting, Clippy, debug/release builds, 36 functional tests, and a 100,000-record benchmark passed. Rust and dependencies are pinned. Linux execution and the automated dependency advisory scan remain unverified. See [the foundation audit](docs/FOUNDATION_AUDIT.md) for evidence and remaining release gates.

## Build and verify

Install Rust and Microsoft C++ Build Tools with the **Desktop development with C++** workload. Bundled SQLite requires a C compiler. From the folder containing `Cargo.toml`:

```powershell
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked --release
cargo test --locked --release --all-targets
```

`rust-toolchain.toml` pins tested Rust 1.99.0; `Cargo.lock` contains actual resolved dependencies. The executable is `target\release\swapctx.exe`. Building may download dependencies; the executable has no network client or telemetry.

The fail-fast Windows script also discovers an exact-version compiler installed outside PATH:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -ReleaseTests -Benchmark
# With cached dependencies, add -Offline; select a cache with -CargoHome.
```

The execution-policy argument applies only to that process. The benchmark is optional and ignored during ordinary tests. Windows/Linux CI and an online `cargo audit` job are supplied but have not run here. Publish the source folder at repository root so GitHub discovers `.github/workflows/ci.yml`.

Optional Node.js 24 schema checks: `node scripts/check-sql.cjs`. These eight supplemental checks do not replace native tests. Node is not a runtime dependency.

## Use

```powershell
$SwapCtx = 'C:\path\to\SwapCtx\target\release\swapctx.exe'
Set-Location 'C:\path\to\your-project'
& $SwapCtx init
& $SwapCtx memory add --kind requirement --key authentication 'Preserve authentication behavior'
& $SwapCtx memory add --kind decision --expires-in-days 30 'Use the existing component architecture'
& $SwapCtx memory search 'authentication'
& $SwapCtx memory recent --limit 10
& $SwapCtx memory get 1
& $SwapCtx memory supersede 1 'Preserve authentication and session expiry behavior'
& $SwapCtx memory recent --include-inactive
& $SwapCtx memory review 2 --expires-in-days 60
& $SwapCtx protect add --path 'src/auth/**' --reason 'Changes need review' --policy ask
& $SwapCtx protect check 'src/auth/login.ts'
# Exit 3: ask matched; no approval was requested or granted.
& $SwapCtx protect list
& $SwapCtx status
& $SwapCtx doctor
& $SwapCtx backup 'C:\path\to\backup\swapctx-snapshot.db'
& $SwapCtx capabilities
& $SwapCtx privacy
```

`init` registers exactly the current folder. Project commands select the nearest registered ancestor. Separately initialized nested projects have separate records/rules; outer rules are not inherited. Relative check targets use the calling directory; rules use the project root.

Global `--data-dir 'C:\path\to\local-data'` selects an independent store. Relative data directories resolve from the calling directory; prefer absolute directories when moving between projects. Default Windows storage is `%LOCALAPPDATA%\SwapCtx\swapctx.db`; non-Windows development uses `$XDG_DATA_HOME/SwapCtx` or `$HOME/.local/share/SwapCtx`.

## Memory contracts

- Search is literal substring matching against content and kind with ASCII-insensitive SQLite LIKE. Percent, underscore, and backslash are literal. Unicode is preserved; Unicode case folding, semantic ranking, embeddings, and full-text indexing are absent.
- Search/recent return newest records first, limits 1–100. Recent uses the project index; substring searches can scan history. Get uses a project-scoped record ID.
- Default retrieval includes active records. `--include-inactive` includes archived/superseded history. `archive` retains data; `supersede` atomically creates a replacement and links the retained original.
- Multiple active records with an identical explicit `--key` display CONFLICT. This does not infer semantic contradictions or decide which statement is correct.
- Expired records display stale and remain visible, including requirements. `--fresh-only` deliberately filters them. `review` records human review; without expiry flags it preserves existing expiry. Use `--expires-in-days` to renew or `--clear-expiry` to remove expiry. The local clock and review timestamps do not prove correctness.
- Potential secrets are rejected on new memory/rule writes unless `--allow-sensitive` is explicit. Known secret-like values are hidden in ordinary output; `memory get ID --reveal` explicitly displays them. Detection has false positives/negatives and is neither comprehensive redaction nor encryption.
- UTF-8 byte bounds: kind/key 128, content 1 MiB, pattern 4 KiB, reason 16 KiB, query 4 KiB. Empty values and NUL fail. Windows command-line limits may be smaller. Existing records are not rewritten or truncated.
- Display escapes terminal controls and directional override/isolate characters while preserving stored content.

## Boundary contracts

Persistent policies are deny, ask, and audit. Omitted policy and legacy rules default to deny. All matches print; deny takes precedence over ask, then audit. Audit emits a diagnostic only, with no persisted event history. Ask opens no approval workflow. No policy intercepts operations.

- Patterns use forward slashes. `src/*.rs` matches one level; `src/**` includes descendants and the directory itself.
- Windows matching is ASCII insensitive, conservatively including case-sensitive directories. Other platforms are case sensitive. Globset uses byte semantics: `?` matches a byte; Unicode classes/casing do not implement full Windows filename equivalence. Prefer literal names or `*` for Unicode paths.
- Checks inspect supplied and resolved in-project names. Internal junction aliases and ordinary absolute paths using short Windows project-root names are supported. Outside destinations, broken links, inaccessible or non-directory ancestors fail. Missing targets use the deepest existing ancestor.
- Traversal, empty targets, controls, alternate streams, reserved devices, ambiguous spaces/dots, drive-relative/root-relative Windows paths, device/verbatim namespaces, and wildcard/invalid Windows target characters fail.
- Checks do not enumerate descendants, cover every hard-link alias, or prevent changes between checking and writing. Check each affected path. No wrapper/hook is installed.
- Identical project/pattern/reason/policy rules fail atomically; different policies/reasons remain allowed. Removal is project scoped.

Exit codes: 0 success or audit-only/no-match; 1 operational/validation/database error; 2 deny match **or** argument-parser usage error; 3 ask without deny. Read diagnostics to distinguish deny from usage errors. Wrappers should stop on every nonzero result; none grants permission to write.

## Storage, migration, recovery

Only init creates a database. Original unversioned and version-1 databases remain readable without modification. Recognized older schemas upgrade transactionally to v2 on init or a write command. Additive migration preserves original IDs, content, timestamps, and indexes; absent metadata means active memory and deny policy. Reads are read-only; other writes open existing databases only.

Unknown, incomplete, foreign, future schemas and unexpected triggers/views fail without resetting stores. Ambiguous legacy ASCII case aliases for Windows projects fail without deleting registrations. Schema checks are not a full content audit; doctor checks integrity, foreign keys, and lifecycle links without repair. Foreign keys and a five-second busy timeout are explicit. SQLite default journal/durability settings remain; existing WAL stores are supported without forcing a journal-mode change.

`backup DESTINATION` creates a consistent **whole-store plaintext** snapshot through SQLite's online backup API, verifies integrity/relationships, flushes, and publishes without overwriting a file. The parent directory must exist. Committed WAL changes are included; uncommitted changes are excluded. Failed staging is cleaned up. A ten-second retry deadline can fail under contention. This is not encryption or a hardware power-loss guarantee.

Restore into a separate local directory by copying a verified snapshot as `swapctx.db`, then run `--data-dir 'C:\absolute\restore-data' doctor` and inspect records. This workflow was tested. No automatic restore/merge command exists. Close database users before replacing an existing store and preserve its entire data directory including journal/WAL/SHM sidecars. Never delete sidecars as repair. Init is not corruption recovery.

## Privacy and next work

Storage/backup is plaintext and inherits local permissions. No secure ACL provisioning, tamper resistance, credential discovery, project scanning, automatic history ingestion, AI calls, or uploads exist. External agents retain their own network behavior. Requirements influence agents only when supplied by a person or a future verified adapter.

Complete online advisory audit and cross-platform CI before production release clearance. Phase 2 should strengthen portable recovery, provenance, and measured retrieval quality through failure injection and a versioned interchange contract. Major agent integrations remain deferred until their real capabilities and interception limits are verified.

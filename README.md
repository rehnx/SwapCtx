# SwapCtx

Local-first external context records and manual project boundary checks. Rust CLI, SQLite storage, Windows first.

SwapCtx preserves requirements, decisions, notes, and rules that you explicitly save. It provides versioned JSON context, an optional managed Codex console, and a context-first exec launcher. It does **not** expand native model context, inspect hidden model state, or intercept agent actions. A check is an observation at a point in time, not enforcement or authorization.

## Foundation status

The [Phase 7B Windows CI fix](docs/PHASE7B_WINDOWS_CI_FIX.md) addresses the hosted
staging-path alias failure and misleading cascading ZIP errors. Containment and
integrity checks are retained; the fix is offline-verified and awaits a hosted
rerun on the patched commit.

[Phase 6 release preparation](docs/PHASE6_RELEASE_APPROVAL.md) now provides one
offline producer command and a PowerShell-only recipient acceptance script.
Receipt-backed packaging independently checks the actual executable imports,
complete source inventory and build helper hashes. No public release, package
submission or persistent user PATH modification has been performed.

The latest [retrieval and session reliability improvements](docs/RETRIEVAL_SESSION_RELIABILITY.md) fix budget selection that discarded smaller useful notes, deduplicate query terms, avoid history reinjection for unchanged selected knowledge, and reject damaged session references before native startup. Storage schema 3, context protocol v1, dependencies and existing command syntax are preserved. Verification remains offline only.

The [authenticated pilot passed eight checks](docs/evidence/codex-pilot-2026-10-17-passed.json) in the user's normal Windows terminal, including six coding stages, resumption and manual compaction. [Phase 5](docs/PHASE5_EVERYDAY_CODEX.md) adds local-only status, an optional live terminal observatory, explicit console memory saves and Codex-home selection. All Phase 5 verification is offline: no Codex invocation, model request or soak test. Ready for opt-in everyday use on the pinned setup; natural automatic recovery and comparative benefit remain unverified.

The retained [Phase 4 reports were audited offline](docs/evidence/phase4-offline-audit-2026-10-17.json): all 6/6 checks per arm passed in the short run; the longer run retained 21 managed and 22 baseline passing turns before `codex_turn_failed`. Neither recorded automatic compaction. The failed turn's root cause is absent from the old report; new console diagnostics retain safe categories without raw backend messages. See [the investigation and everyday commands](docs/PHASE5_EVERYDAY_CODEX.md).

Phase 3 provides the managed console's startup, resume and compaction transport. The later authenticated pilot demonstrates context use and tested requirement adherence for those particular turns, without a general guarantee. Native TUI hook delivery remains unverified. See [Phase 3 usage, evidence and limits](docs/PHASE3_CODEX_LIFECYCLE.md). [Context protocol v1](docs/CONTEXT_PROTOCOL.md), [the earlier readiness milestone](docs/CODEX_READINESS.md), [Phase 2 verification](docs/PHASE2_RELIABILITY.md) and [the Phase 1 audit](docs/FOUNDATION_AUDIT.md) document the foundation. Schema 3 and dependencies remain unchanged. Linux execution and automated dependency advisory scanning remain unverified.

## Build and verify

For portable Windows packaging, run the deterministic verification below, then:

```powershell
node .\scripts\build-windows.cjs .\.build\windows-release --test
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\package-windows.ps1 `
  -Binary .\.build\windows-release\swapctx.exe `
  -BuildInfo .\.build\windows-release\build-info.json `
  -Destination .\.build\windows-candidate
```

The destination must be new and its parent must exist. This creates a ZIP,
an offline `install-windows.ps1`, and SHA256SUMS.txt from the existing release binary, with an internal file-hash
manifest and a [self-contained Windows quickstart](docs/WINDOWS_RELEASE.md).
Packaging does not build or certify the binary; always verify the current source
first. It includes no working database, credentials, logs or test reports.
Checksums detect corruption but do not authenticate the publisher. The package
is an unsigned local candidate; dependency redistribution review, advisory
audit, hosted CI and clean-machine validation remain release gates.
Cargo metadata now matches the repository's existing Apache-2.0 LICENSE.

The offline Windows release recipe compares two isolated builds, links the C
runtime statically, and records source hashes plus PE imports. The same-machine
comparison has passed; cross-machine reproducibility remains unverified. Direct
`cargo build --release` remains supported for development and may require the VC
runtime. See [release build and command discovery](docs/RELEASE_BUILD_READINESS.md)
for prerequisites, results, session PATH setup and current preview blockers.

The archive also includes generated third-party notice texts verified against
Cargo.lock. See [preview distribution hardening](docs/PREVIEW_DISTRIBUTION.md)
for installation, notice regeneration and the remaining release gates.

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
# Offline deterministic checks only, rejecting agent options and skipping soak checks:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -DeterministicOnly -ReleaseTests
```

The execution-policy argument applies only to that process. The benchmark is optional and ignored during ordinary tests. Windows/Linux CI and an online `cargo audit` job are supplied but have not run here. Publish the source folder at repository root so GitHub discovers `.github/workflows/ci.yml`.

Optional Node.js 24 contract checks: `node scripts/check-sql.cjs`, `node scripts/check-context.cjs target/release/swapctx.exe`, `node scripts/check-session.cjs target/release/swapctx.exe` and `node scripts/check-everyday.cjs target/release/swapctx.exe`, or add `-ContextTests` to verification. Add `-CodexPath 'C:\absolute\codex.exe'` for the isolated loopback Codex 0.162.1 acceptance suites. `-ModelTests` additionally opts into the separate real-provider context-use evaluation using generated probe data and Codex's configured authentication. These checks supplement native tests. Node is only a runtime dependency of the optional Codex adapters; ordinary Rust memory commands remain standalone. `-DeterministicOnly` forbids all agent-test options.

`-ContextTests` also runs `scripts/check-pilot-harness.cjs` without provider requests. Explicit `-PilotTests -CodexPath 'C:\absolute\codex.exe'` adds the authenticated disposable coding pilot. See [pilot instructions and blocked evidence](docs/REAL_WORLD_CODEX_PILOT.md).

`-ContextTests` now includes the longer-session evaluator's offline self-tests. Real longer-session comparisons remain explicit: use `scripts/check-soak.cjs` with the absolute Codex executable, SwapCtx executable and a new report path. See [Phase 4 execution, costs and result gates](docs/PHASE4_LONG_SESSIONS.md#run-outside-the-restricted-tool-process). No manual compaction or lowered threshold is used to manufacture automatic-recovery evidence.

## Machine context and Codex handoff

```powershell
& 'C:\path\to\swapctx.exe' --data-dir 'C:\path\to\data' context `
  --protocol-version 1 --query 'architecture authentication' --max-bytes 8192
```

Context packets prioritize requirements, report self-reported origins, omit invalid/stale/conflicting/secret-like records by default, and expose missing mandatory coverage. Byte budgets include the complete output, rather than estimating model tokens. Existing human-readable search semantics are preserved. See the [protocol](docs/CONTEXT_PROTOCOL.md) and [opt-in launcher usage](docs/CODEX_READINESS.md#explicit-launcher-use). No hooks are automatically installed. Using the launcher forwards selected context to your configured Codex provider; retrieval itself stays local.

For the managed console, from a registered project or a nested directory:

```powershell
& 'C:\path\to\swapctx.exe' codex setup --codex 'C:\absolute\path\to\codex.exe'
& 'C:\path\to\swapctx.exe' codex status
& 'C:\path\to\swapctx.exe' codex run --observe
# In a second terminal within the same registered project:
& 'C:\path\to\swapctx.exe' codex watch
```

This optional console requires Node 24+ and Codex 0.162.1. It resumes the project's saved managed thread and refreshes selected context before each turn. `/help` lists local memory, status and observation commands. `run --new` starts another thread while preserving the previous reference. Read-only is the default; setup's `--workspace-write --replace` explicitly enables editing, and optional `--codex-home ABSOLUTE_DIRECTORY` selects an existing native home. Setup leaves native Codex configuration and hooks unchanged. Ordinary `codex` launches do not activate this integration. See [everyday usage and limits](docs/PHASE5_EVERYDAY_CODEX.md). `codex status` and `codex watch` are local-only; `codex doctor` starts Codex without a model request.

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
& $SwapCtx memory history 1
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
& $SwapCtx export 'C:\path\to\backup\project-context.db'
# From the destination registered project:
& $SwapCtx import 'C:\path\to\backup\project-context.db'
& $SwapCtx import 'C:\path\to\backup\project-context.db' --apply
& $SwapCtx --data-dir 'C:\path\to\NEW-recovery-data' restore 'C:\path\to\backup\swapctx-snapshot.db'
& $SwapCtx capabilities
& $SwapCtx privacy
```

`init` registers exactly the current folder. Project commands select the nearest registered ancestor. Separately initialized nested projects have separate records/rules; outer rules are not inherited. Relative check targets use the calling directory; rules use the project root.

Global `--data-dir 'C:\path\to\local-data'` selects an independent store. Relative data directories resolve from the calling directory; prefer absolute directories when moving between projects. Default Windows storage is `%LOCALAPPDATA%\SwapCtx\swapctx.db`; non-Windows development uses `$XDG_DATA_HOME/SwapCtx` or `$HOME/.local/share/SwapCtx`.

## Memory contracts

- Search is literal substring matching against content and kind with ASCII-insensitive SQLite LIKE. Percent, underscore, and backslash are literal. Unicode is preserved; Unicode case folding, semantic ranking, embeddings, and full-text indexing are absent.
- Search/recent return newest records first, limits 1–100. Recent uses the project index; substring searches can scan history. Get uses a project-scoped record ID.
- Default retrieval includes valid active records. `--include-inactive` includes invalid/archived/superseded history. `archive` retains data; `supersede` atomically creates a replacement and links the retained original. Supersession preserves expiry unless `--expires-in-days` or `--clear-expiry` explicitly changes it.
- `memory invalidate ID --reason 'Evidence contradicts this statement'` retains invalid knowledge and its reason but excludes it from default retrieval. Review records a review event and does not undo invalidation; save a corrected replacement through supersession.
- `memory add --source 'requirements.md:section-4' --actor 'maintainer' TEXT` records self-reported origin labels (defaults manual/user). Stable origin IDs survive transfers. `memory history ID` shows retained lifecycle events. Legacy records explicitly have unknown origins/prior history. No claim is made that labels are authenticated or history tamper-proof.
- Multiple valid active records with an identical explicit key display CONFLICT. This does not infer semantic contradictions or choose a correct statement. Search supports exact `--kind` and `--key` filters; `--unconflicted` excludes competing keys. Stale valid active records still count as competing knowledge. Conflict exclusion aggregates keys once instead of recounting each candidate.
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

Only init creates the ordinary working database. Restore creates an explicitly selected new store; export creates a separate bundle. V0/v1/v2 stores remain readable without modification. Recognized older schemas upgrade transactionally to v3 on init or a write command. Migration preserves original IDs, content, timestamps, indexes, and lifecycle metadata; it registers unknown origins instead of inventing earlier history. Reads are read-only; ordinary writes open existing stores only.

Before an upgrade, use backup to retain a pre-migration snapshot. Older SwapCtx binaries reject schema 3; migration is backward compatible with existing data, not a supported downgrade. The new binary can back up a legacy store without migrating it.

Unknown, incomplete, foreign, future schemas and unexpected triggers/views fail without resetting stores. Ambiguous legacy ASCII case aliases for Windows projects fail without deleting registrations. Schema checks are not a full content audit; doctor checks integrity, foreign keys, and lifecycle links without repair. Foreign keys and a five-second busy timeout are explicit. SQLite default journal/durability settings remain; existing WAL stores are supported without forcing a journal-mode change.

`backup DESTINATION` creates a consistent **whole-store plaintext** snapshot through SQLite's online backup API, verifies integrity/relationships, flushes, and publishes without overwriting a file. The parent directory must exist. Committed WAL changes are included; uncommitted changes are excluded. Failed staging is cleaned up. A ten-second retry deadline can fail under contention. This is not encryption or a hardware power-loss guarantee.

`--data-dir 'C:\absolute\NEW-directory' restore SNAPSHOT` validates before creating a new directory, then makes a verified online snapshot into it. The parent must exist; any existing directory is refused, including empty ones. Default storage cannot be a restore target. No working store is replaced or merged. Interruption/failure may leave the new directory or temporary staging files; inspect them and choose a different new directory for retry. Doctor checks the restored store. Close database users before manually replacing an existing store; preserve its whole directory including journal/WAL/SHM sidecars. Never delete sidecars as repair. Init is not corruption recovery.

`export FILE` creates a fresh, schema-3 SQLite bundle containing only the selected project, its rules, all memory/history, and original project path. It never copies unrelated projects or their freed database pages. Run init on legacy stores first to establish stable origins. Existing export files are refused. Bundles are plaintext, include potentially sensitive saved history and provenance, and are not signed.

`import FILE` previews without writes/migration. `--apply` merges atomically into the current registered project, remapping local IDs and supersession links. Stable origins deduplicate identical copies; divergent content, lifecycle metadata or non-import history refuses the entire merge, as does an origin owned by another local project. No conflict is silently resolved. Existing rules are retained; identical rules skip. Repeated import adds no duplicate history. Inputs must contain exactly one project, pass schema/integrity/FK/history checks and input/secret guards. `--allow-sensitive` explicitly permits secret-like values. No SQL dumps are executed. Apply may first migrate a recognized legacy destination through the normal write-open path.

Transfer limits: 256 MiB input file, 100,000 rows and 64 MiB decoded values per table. Larger stores can use whole-store backup/restore. These are practical bounds, not a sandbox against every malicious SQLite file. Connections disable trusted schemas and enable SQLite defensive mode. There is no repair command for corrupt snapshots or automatic divergence resolver.

## Privacy and next work

Storage/backup is plaintext and inherits local permissions. No secure ACL provisioning, tamper resistance, credential discovery, project scanning or automatic history ingestion exists. The Rust CLI has no AI/network client. Optional Codex adapters supply external records to Codex; Codex retains its own network/logging behavior. Delivery does not guarantee requirements will be followed. Memory backup/export does not include Codex adapter settings/session references or native Codex conversation history.

Complete online advisory audit and cross-platform CI before production release clearance. The managed console has prior authenticated pilot evidence and Phase 5 offline regression coverage for opt-in everyday use on the pinned setup. Natural automatic recovery and measured comparative benefit remain unverified. Native TUI lifecycle delivery and tool protection remain unverified. See the [current readiness decision](docs/PHASE5_EVERYDAY_CODEX.md#verification-and-readiness); no additional expensive evaluation was run for this milestone.

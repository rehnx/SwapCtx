# Phase 1 foundation audit and verification

Historical Phase 1 baseline. The current implementation uses schema 3 and adds provenance, invalidation and guarded transfer/recovery; see [Phase 2 results](PHASE2_RELIABILITY.md) for current functionality and limitations. Statements below describe the earlier Phase 1 revision.

Updated 2026-10-12. Scope: all supplied implementation, Rust modules, SQL schemas, tests, fixtures, verification scripts, and documentation. No separate specification, AGENTS.md, or Git history was found. The user's objectives govern this work. No agent integrations were added.

## Outcome

The native Windows foundation is built and tested. Rust was installed in the user profile but absent from PATH; locating it and using a workspace-local dependency cache enabled offline verification. The earlier report that native verification was blocked is superseded. Cargo.lock now contains actual resolved dependencies; Rust 1.99.0 is pinned.

Production release clearance remains conditional: automated advisory scanning could not run offline, Linux/hosted CI have not executed, and hardware/storage fault guarantees are not established. These are open gates, not reported successes.

## Findings and improvements

Rust and bundled SQLite remain suitable for a small local CLI. Modules isolate orchestration, storage, memory, boundaries, paths, secrets, display, and backups without a framework rewrite. Parameterized SQL, literal LIKE escaping, project isolation, and original project/recent indexes remain. Measured behavior did not justify FTS or an embedding dependency.

| Finding | Improvement and practical boundary |
|---|---|
| Boundaries had one implicit outcome | Persistent deny/ask/audit policies, deterministic precedence, duplicate rejection, legacy deny defaults. Ask grants no approval; audit emits diagnostics without saving events; no enforcement. |
| Memory lacked lifecycle metadata | Explicit keys/conflicts, expiry/review, archive, atomic supersession with retained history. Stale requirements stay visible by default. Concurrent replacement permits one winner without partial records. |
| Nested lookup and Windows case identity were unreliable | Indexed nearest-ancestor discovery and isolated nested projects. Legacy ASCII case collisions fail without data deletion. |
| Short absolute Windows root names falsely appeared external | Normalize the shallowest ancestor identifying the canonical root, preserving names below it. Actual Windows tests cover short names, internal/external junctions, and a junction pointing back to the root. |
| Paths/globs could mislead checks | Separator semantics, ASCII Windows casing, subtree-directory matches, fallible compilation, lexical/resolved checks, fail-closed link inspection, reserved/stream/namespace/ambiguous-path rejection. Unicode equivalence, hard links, and check/write races remain limited. |
| Unversioned schema and permissive opens endangered stores | Transactional additive v2 migration, application ID/version, structural/FK checks, read-only reads, existing-only writes, rejection of unknown/future stores. V0/v1 reads do not migrate. Real migration DDL failure rolls back preserving originals. |
| Controls/secrets could reach output unnoticed | Terminal/bidirectional escaping, input bounds, heuristic secret guard with override, default output hiding and explicit reveal. Original records remain unchanged. Not encryption or comprehensive redaction. |
| Live file copying risked inconsistent backups | SQLite online snapshots, same-directory staging, copied-store diagnostics, flush, no-overwrite publication, bounded retries and cleanup. Actual WAL isolation and separate restored-store reopening passed. |
| Errors/capabilities were opaque | Read-only doctor checks integrity/FKs/lifecycle and reports project lookup failures; capabilities works without a store. No hidden-state access, native expansion, enforcement, or integrations claimed. |
| No reproducible native/performance evidence | Pinned compiler/lockfile, fail-fast script, debug/release suites, real Windows junctions, contention/concurrency tests, opt-in 100,000-record process benchmark. |

Original wildcard semantics intentionally changed: star stays within one level, double-star recurses. Review saved rules on upgrade. Unsupported legacy patterns fail explicitly and can be listed/removed. Existing data is not rewritten. Ask adds exit 3; deny remains 2, also used for parser usage errors.

## Verification evidence

Windows x86_64 MSVC, Rust/cargo 1.99.0, native bundled SQLite 3.53.2. Tests use isolated stores, not the user's real database.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -Offline -ReleaseTests -Benchmark
```

| Gate | Executed result |
|---|---|
| cargo fmt --check | Passed |
| cargo clippy --locked --offline --all-targets -- -D warnings | Passed, no warnings |
| cargo test --locked --offline --all-targets | 14 unit + 22 CLI process tests passed; benchmark ignored |
| cargo build --locked --offline --release | Passed; executable 3,920,384 bytes (3.74 MiB) |
| cargo test --locked --offline --release --all-targets | Same 36 functional tests passed |
| Opt-in release benchmark | Passed with 100,000 records |
| node scripts/check-sql.cjs | Eight supplemental schema checks passed, SQLite 3.53.0 |

Functional coverage: literal wildcard/injection/Unicode retrieval and persisted round trips; nested isolation; direct lookup; input limits; terminal escaping; rule matching/policy precedence/scoped removal; Windows casing/short roots/junction aliases/escapes; secret rejection/override/hiding/reveal; real v0/v1 migrations preserving IDs/timestamps; future/foreign/corrupt stores; DDL-failure rollback; lifecycle/history/freshness/review/conflicts; scoped mutations; concurrent supersession; five-second lock exhaustion then recovery; immutable no-overwrite backups; WAL committed/uncommitted snapshot isolation; separate restored-store isolation; cross-project replacement detection and failed-backup cleanup.

Supplemental checks execute actual v2 SQL and the original fixture: migration preservation/idempotence, policy/orphan rejection, state/expiry constraints, supersession links, key bounds, actual DDL rollback, integrity and FKs. They are not a substitute for native tests.

### Retrieval measurements

The release benchmark inserts 100,000 records of approximately 256 bytes of context plus each record identifier. Twenty samples include the complete CLI process and database validation/opening, not just SQL:

| Command | p50 | p95 |
|---|---:|---:|
| Recent, default limit | 18.61 ms | 26.74 ms |
| Absent literal substring search | 88.24 ms | 164.62 ms |
| Get oldest record by ID | 26.93 ms | 30.29 ms |

These are observations on this Windows machine, not guarantees. Corpus metadata is sparse/default, storage local, and there is one project. Lifecycle-heavy metadata, many projects, larger content and concurrent workloads need further measurement. Substring search remains a scan; recency retains the project index and ID lookup uses the primary key.

## Dependencies and outstanding gates

[The dependency inventory](DEPENDENCIES.md) records 49 resolved third-party packages and declared licenses from Windows-filtered locked metadata. All declare licenses. This is an inventory, not legal clearance or a complete all-platform SBOM.

Cargo-audit is not installed. Offline installation failed because its crate was absent from cache; external downloads were unavailable. Limited primary RustSec review does not constitute a full scan or a vulnerability-free claim. Online CI installs cargo-audit and scans Cargo.lock. Windows/Linux hosted CI has not run; the supplied package has no Git repository.

Before production release:

- Execute Windows/Linux hosted CI and online advisory audit; triage findings. Non-Windows dependency availability was not verified offline.
- Expand failure injection for disk-full, permission failures, process interruption, migrations and snapshot publication. SQLite transactions/backup semantics do not prove all power-loss or filesystem failure behavior. Actual contention, rollback, corruption rejection and restore reopening passed.

Other limits:

- Plaintext storage/backup inherits local permissions. No encryption, secure ACL setup, tamper evidence, secure deletion, or defense against hostile same-account file replacement. Secret detection has false positives/negatives.
- Windows matching uses ASCII casing and byte globs, not complete NTFS equivalence. Checks do not intercept writes, enumerate recursive effects, cover all hard-link names, or eliminate races. Unix symlink branches await Linux execution.
- Project identity follows canonical location. Moving projects/merging case aliases has no automated workflow. Nested rules are not inherited.
- Key conflicts are same-key competition, not inferred contradictions. Expiry uses the system clock. No provenance source records, versioned export/import, automatic restore/merge, semantic/full-text retrieval or persistent operation audit trail.
- No hidden-state inference, native context expansion, automatic context capture/replay, agent integration or enforcement exists.

## Recommended Phase 2

Close the external release gates first. Then prioritize data portability and evidence: versioned export/import with provenance, guarded restore, recovery failure injection, and realistic lifecycle-heavy retrieval evaluation. Major integrations remain later work requiring verified platform capabilities and operation-level tests distinguishing supplied context, observed actions, and actual supported interception.

## Primary technical references

- [Microsoft Windows path naming](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file): namespaces, reserved devices and path ambiguity.
- [Globset builder](https://docs.rs/globset/latest/globset/struct.GlobBuilder.html) and [source](https://docs.rs/crate/globset/latest/source/src/glob.rs): separator, byte, casing and matcher behavior.
- [Rusqlite 0.40.2](https://docs.rs/rusqlite/0.40.2/rusqlite/) and [SQLite backup API](https://www.sqlite.org/backup.html): connections, transactions and consistent snapshots.
- [RustSec rusqlite records](https://rustsec.org/packages/rusqlite.html): partial manual review, not a full lockfile audit.

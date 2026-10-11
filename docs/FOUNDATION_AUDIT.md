# Phase 1 foundation audit

Audit date: 2026-10-11. Scope: the complete supplied source package (`Cargo.toml`, one original Rust source file, four original unit tests, README, license, and ignore rules). No separate product specification, `AGENTS.md`, Git history, or existing build artifacts were found. Product objectives and non-negotiable principles in the task are the governing requirements.

## Outcome and release gate

Foundation changes are implemented, with no agent integrations or major new features. **This revision is not certified as buildable or production ready.** Rust is absent, and the attempt to obtain a toolchain from `static.rust-lang.org` failed because the environment could not connect. `cargo test --all-targets` was attempted and failed before running tests (`cargo` not recognized). There was no automatic approval rejection or permission prompt; external toolchain access simply failed.

Eighteen supplemental checks passed under Node.js v24.17.0 / SQLite 3.53.0 using SQL extracted from the actual Rust source. These are database-level evidence, not a substitute for compiling the application. The Windows/Linux Rust CI workflow and the executable tests are authored but unexecuted. The supplemental script also passed `node --check`.

## Vision and architectural judgment

SwapCtx's current truthful scope is a local external-memory store and manual path-rule checker. Persistent requirements and explicit decisions can be useful without model access, but persistence alone does not ensure agent retrieval, adherence, or context recovery. Protection requires a real interception surface before an action, while debugging requires observable evidence of supplied context and subsequent behavior. The tool has neither today.

Rust and bundled SQLite remain appropriate for a small Windows CLI: no service, network client, API key, embedding runtime, or new production dependency is needed for this phase. The original parameterized SQL, project-scoped queries, escaped LIKE terms, explicit privacy boundaries, and deepest-existing-ancestor resolution were useful foundations and were preserved. SQLite already indexes per-project recent retrieval through the existing index and rowid ordering; a redundant index or search rewrite was not justified.

The source was separated into CLI orchestration, storage lifecycle, path/rule handling, and terminal display. This isolates security boundaries for testing without a framework or large architectural rewrite. `tempfile` is a development-only dependency for isolated fixtures. A global explicit data directory is the one small CLI addition, needed to make process tests independent of the user's real store. A nonunique ASCII case-insensitive project-path index supports Windows project selection without scanning registered projects; the original recent-memory index is preserved.

## Findings and changes

| Finding | Impact | Change / remaining boundary |
|---|---|---|
| Case-sensitive default globs on Windows | Ordinary case variants could miss saved rules | ASCII case-insensitive matching on Windows. Full Unicode/NTFS equivalence remains unsupported. |
| Default `*` crosses separators | Rules have surprising breadth | Explicit separator semantics: `*` stays within one level, `**` is recursive. This is an intentional behavior correction. |
| Subtree glob misses its directory | Checking `src/auth` could miss `src/auth/**` | Include the subtree directory in the same matcher. General descendant enumeration remains absent. |
| Canonical destination alone loses link alias | An alias-protection rule could miss an internal link | Check both lexical and canonical names; reject outside, broken, or inaccessible destinations. Hard links and check/write races remain. |
| Windows ambiguous names insufficiently checked | Alternate streams, drive-relative paths, device paths, and normalized names could yield misleading checks | Reject unsupported representations, reserved names, leading spaces, trailing dots/spaces, wildcard check targets, and traversal. Absolute ordinary in-project paths remain supported. |
| `exists()` hides inspection errors | Permission errors or broken links may be treated as missing paths | Use `symlink_metadata`, handle only NotFound as missing, require the resolved ancestor to be a directory. |
| Lossy Unicode project identity | Distinct non-Unicode paths can collapse to one stored identity | Refuse lossy identity conversion. UTF-8 text remains intact. Reject ambiguous legacy ASCII case aliases on Windows without deleting records. Windows non-ASCII case aliases remain unverified. |
| Lookup only works at exact registered folder | Commands from project subdirectories fail | Select nearest registered ancestor; nested projects retain separate records/rules. No rule inheritance is implied. |
| Read path uses a create-capable database open after existence check | A deletion race can create an empty database | Explicit read-only and existing-only write flags. Only init has CREATE permission. |
| Schema creation is unversioned and nontransactional | Partial initialization and future-version collisions | Transactional initialization, application ID, schema version, legacy recognition, column/FK/unique checks, rejection of unexpected triggers/views. Unknown schemas are never silently reset. |
| Default lock behavior implicit | Contention policy depends on library defaults | Explicit five-second busy timeout; preserve journal/durability defaults. Real Rust concurrency tests await execution. |
| Saved strings printed as terminal instructions | Escape/control sequences can alter displays or forge lines | Escape terminal controls and selected bidirectional controls on every dynamic output/error path. Preserve raw records. |
| Unbounded/empty values | Accidental storage growth, empty full-scan searches, confusing output | UTF-8 byte bounds, nonempty validation, NUL rejection, bounded results, streaming records instead of collecting all results. No arbitrary legacy truncation. |
| Rule duplication permitted | Repeated checks/output and avoidable rule growth | Atomic duplicate check for identical project/pattern/reason. Preserve rules with different reasons. |
| Rule matcher compilation assumes success | Untrusted pattern compilation could follow a library panic path | Use fallible `GlobSetBuilder::build` and validate before saving. |
| Deep `process::exit` during rule checks | Skips Rust destructors and hinders tests | Return a command exit code through `main`/`ExitCode`. Existing 0/1/2 contract retained, with usage-error ambiguity documented. |
| Four unit tests, no process or migration coverage | Persistence/isolation/exit-code behavior lacked evidence | Expanded unit tests, original-schema fixture, real executable tests, supplemental SQL checks, and Windows/Linux CI. Rust execution remains blocked. |
| No lockfile or release evidence | Dependency/build reproducibility unestablished | Document lockfile generation/commit and locked builds as release gates. Do not invent a lockfile without dependency resolution. |
| Manual copying of a live database suggested | Backups may be inconsistent | Document closed-store whole-directory backups and preserving sidecars. Live backup/restore tooling deferred. |

## Verification evidence

Executed from the crate directory:

```powershell
node scripts/check-sql.cjs
```

Result: **18 passed**, covering fresh/idempotent schema creation; unique project identity validation SQL (including rejection of partial uniqueness); indexed ASCII case-insensitive project lookup; ambiguous case identity detection SQL; literal `%`, `_`, and backslash retrieval with Unicode content; ASCII kind matching; project search isolation; SQL injection as literal input; recent ordering/limit; index use without a temporary sort; rejection of orphan inserts; preservation of legacy IDs/content/indexes; transactional rollback of DDL/version changes; project-scoped rule removal; SQLite integrity/foreign-key checks; file-backed persistence after closing/reopening; read-only write rejection; and lock contention without partial writes followed by successful writing after rollback.

The SQL script uses in-memory databases, the original-schema fixture, and an isolated temporary on-disk database that it cleans up. It does not touch the per-user store. It checks the SQL text itself and does not call Rust validation functions or the CLI. Its contention fixture uses a short SQLite timeout, not the application's five-second setting. Node's SQLite version may differ from rusqlite's bundled version; the Rust suite must verify the latter.

Authored Rust coverage includes literal search/persistence across separate processes, Unicode round trips, nested-project selection/isolation, relative/absolute target handling, rule exit codes, duplicate rules, removal isolation, invalid input and parser limits, terminal controls without stored-content changes, actual legacy migration with stable IDs, corrupt/foreign/future databases, invalid stored rules, concurrent writers, transaction rollback, Windows case/ambiguous inputs and junction aliases/escapes, and Unix symlink equivalents. None is reported as passed.

Reproducible remaining checks on a Rust-equipped machine:

```powershell
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

After successful resolution, commit `Cargo.lock`, pin a tested Rust toolchain, switch CI formatting to `--check`, and run builds/tests with `--locked`. The workflow has not been submitted to or executed by GitHub; the supplied package has no Git repository.

## Remaining limitations and unverified behavior

- Build, Rust API/type correctness, Clippy, rustfmt, real CLI behavior, and the Windows/Linux CI matrix are unverified. This is the first release gate, not optional follow-up testing.
- Dependency vulnerability/license review and a deterministic lockfile remain incomplete. No security-audit success is claimed.
- Storage is plaintext, inherits directory permissions, and has no tamper resistance. DPAPI, key recovery, secure ACL provisioning, or protection from same-account processes are not implemented.
- Manual checks are snapshots of one supplied path. Directory operations, all hard-link names, reparse changes between check/write, all Unicode case mappings, and alternate namespace equivalence are not comprehensively covered. No actual write is intercepted.
- Legacy schema checks do not automatically validate every data row or repair corruption. Power-loss, disk-full, permissions, interrupted writes/migrations, lock exhaustion, and real backup restoration need fault-injection tests. SQLite transaction semantics are evidence for atomicity, not a claim that all hardware/storage failures have been tested.
- Records remain scoped by canonical project location; moving a project does not migrate its identity automatically. Nested projects override selection and do not inherit outer rules. Full Windows Unicode case identity equivalence remains unsupported.
- Literal substring search can scan a project's entire memory history. No semantic relevance, FTS index, retention policy, memory update/delete/export workflow, or measured large-store performance guarantee is implemented. Result limits constrain returned results, not scan cost.
- Original unversioned databases remain usable until init upgrades them. Advanced unsupported legacy rule patterns now cause explicit errors; users can list/remove those rules. Existing content is preserved. Rule wildcard semantics intentionally changed and must be reviewed on upgrade.
- No native context expansion, hidden-state inference, automatic capture/replay, agent-hook enforcement, or claimed platform integration exists. The old speculative mention of Codex session APIs has been removed from the roadmap; adapters require capability verification first.

## Recommended next phase

Finish the Phase 1 release gate first: compile, fix any failures, execute the full matrix, commit formatting and lockfile, review dependencies, and reproduce Windows filesystem and storage-failure tests. Do not add agent features while this gate is open.

Then build a local data durability and recovery phase: consistent live backup, tested restore, explicit integrity diagnostics, versioned export/import, deliberate memory update/removal with provenance, and measured retrieval behavior on realistic histories. Only after those work should one documented agent adapter be implemented, distinguishing supplied external context, observed events, and any platform-supported interception. Evidence-based debugging should describe those observable events rather than attributing failures to hidden model state.

## Primary technical references checked

- [Microsoft: Naming Files, Paths, and Namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file) and [filename whitespace behavior](https://learn.microsoft.com/en-us/troubleshoot/windows-client/shell-experience/file-folder-name-whitespace-characters): reserved devices, namespace/drive-relative paths, and filename normalization constraints.
- [globset GlobBuilder](https://docs.rs/globset/latest/globset/struct.GlobBuilder.html) and [globset source](https://docs.rs/crate/globset/latest/source/src/glob.rs): separator defaults, byte-based patterns/ASCII case handling, and the infallible matcher compilation expectation. These informed explicit settings and fallible set compilation.
- [rusqlite 0.40.2 Connection](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Connection.html), [OpenFlags](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.OpenFlags.html), and [Transaction](https://docs.rs/rusqlite/0.40.2/rusqlite/struct.Transaction.html): existing/read-only database opening, explicit busy timeout, and transaction rollback/borrowing APIs. Documentation review is not compilation evidence.

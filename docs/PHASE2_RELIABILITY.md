# Phase 2: context reliability and recovery

Historical Phase 2 baseline. The subsequent [Codex readiness milestone](CODEX_READINESS.md) adds a machine-readable contract and tested explicit exec handoff; its readiness assessment supersedes the integration recommendation below.

Implemented 2026-10-12. This builds on the tested Windows Phase 1 foundation; no external service, agent integration, or new dependency was added. SQLite remains the portable data format, identified by SwapCtx application ID and schema version 3. This avoids an additional parser/serialization stack and permits independent local inspection.

## What changed

- Schema 3 adds stable random origin IDs, self-reported source/actor labels, retained lifecycle events, validity and invalidation reasons. Migration is additive and transactional. Original IDs/content/timestamps, expiry, links and policies survive. Prior origins/history are explicitly unknown, with a migration-registration event; timestamps do not invent when legacy knowledge was created.
- Add, review, archive, supersede and invalidate commit their record changes and history together. Supersession links history by stable origins and preserves expiry unless explicitly changed. Invalid knowledge remains inspectable but is excluded from ordinary retrieval. Reviewing invalid knowledge does not reactivate it. Local history is not authenticated or tamper-proof.
- Project export builds a fresh SQLite bundle, preserving history while excluding other projects and their freed pages. Export requires migrated origins, verifies the result, flushes, and publishes without overwriting a file. It includes archived/invalid/superseded information and the original root path; it is deliberately not redacted or encrypted.
- Import previews without writing. Apply merges in one immediate transaction, preserves origin IDs/events/timestamps, remaps IDs/links, and skips exact duplicates. Existing-origin divergence refuses the whole merge. Imported events retain their timestamps; a local import event records the receipt. Import events are excluded only from identity comparison so repeated transfers deduplicate; all stored import events remain in history. Rules merge without removing existing rules. Preview/apply run schema, integrity, lifecycle, bounds and secret checks. Apply's normal destination migration, if needed, is a separate atomic migration before merge.
- Restore validates a snapshot before creating a new explicitly selected data directory. Existing directories and the default store cannot be replaced. Consistent backup publication remains staged and no-overwrite. A failed/interrupted restore may leave its newly created directory/staging files, and never automatically deletes them or overwrites an existing store.
- Retrieval adds exact kind/key filtering and exclusion of explicit conflicts. Invalid knowledge does not create active-valid conflicts; stale knowledge remains visible and competing until deliberately addressed. Conflict filtering uses one non-correlated aggregate; displayed counts are cached by key within each result set. Supersession cycles are checked through linear graph traversal rather than numeric-ID ordering, supporting legitimate remapped links.
- SQLite connections disable trusted schemas and enable defensive mode. Imports read typed values through parameterized queries; they never execute supplied SQL dumps. New provenance/reasons obey input bounds and secret guards. Origins/events/expiry types and relationships have additional diagnostics.

## Reproduce verification

From the crate directory, with the pinned Rust toolchain and available dependencies:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -Offline -ReleaseTests -Benchmark
node scripts/check-sql.cjs
```

Omit Offline on a connected machine without a populated cache. The benchmark is opt-in and has no fragile CI timing threshold. All tests use temporary databases and do not touch the per-user store. See the final results below for executed evidence, rather than treating the command list as a success claim.

## Acceptance coverage

New native tests cover self-reported labels and history, invalidation/default exclusion, exact filters/conflict resolution through invalidation, project isolation, v2 read compatibility, real v3 migration failure with rollback of ALTER TABLE and original data, retained unknown provenance, project-only export without unrelated plaintext, file no-overwrite, preview no-write, preserved origins/timestamps/history, remapped supersession links, repeated and concurrent import deduplication, divergence refusal, secret guards, hostile triggers, corrupt/oversized inputs, multi-project import refusal, guarded restore/reopen/isolation, cycle/missing-origin diagnostics and refusal to publish invalid backups/exports.

Two concrete failure-resilience checks are included:

1. SQLite max_page_count forces an actual SQLITE_FULL during import. Memory/metadata/origin/event/rule tables remain unchanged; integrity succeeds, and retry after lifting the limit succeeds. This models pager space exhaustion, not every physical disk failure.
2. A 10,000-record import subprocess is killed after its plan is printed while its write transaction holds a database lock. Reopening recovers the original record, origins and history, with doctor passing. This is one real process-interruption point, not exhaustive crash/power-loss coverage.

A partial bundle followed by a complete bundle verifies a remapped supersession link to a lower local ID; numeric ordering is not treated as proof of valid history.

## Executed results

The complete fail-fast verification script above exited 0 on Windows x86_64 MSVC, Rust/cargo 1.99.0, bundled SQLite 3.53.2:

| Gate | Result |
|---|---|
| Format check | Passed |
| Clippy, all targets, warnings denied | Passed |
| Debug native tests | 15 unit + 32 CLI tests passed; one opt-in benchmark ignored |
| Locked offline release build | Passed |
| Release native tests | Same 47 functional tests passed |
| Opt-in release benchmark | Passed |
| Supplemental Node schema checks and syntax check | Passed; 10 checks, SQLite 3.53.0 |

The performance fixture contains 100,000 memories with origins, add events, and explicit-key metadata (50,000 competing pairs). Each record contains approximately 256 bytes of context plus its identifier. Twenty complete CLI subprocess measurements per operation:

| Operation | p50 | p95 |
|---|---:|---:|
| Recent | 12.17 ms | 17.40 ms |
| Absent literal substring search | 72.54 ms | 122.75 ms |
| Get oldest record including provenance/history | 35.98 ms | 44.29 ms |
| Search excluding all competing keys | 184.51 ms | 242.90 ms |

These are this machine's measurements, not guarantees or a controlled speed comparison with Phase 1. This tests populated metadata, but not semantic accuracy, many-project workloads, arbitrarily long history or every concurrency pattern. Result filtering is validated by functional fixtures independently of timing.

## Remaining limits and integration recommendation

- Hosted Windows/Linux CI and the automated advisory audit remain unexecuted. The auditor is absent from the offline cache; no vulnerability-free claim is made. Dependency inventory/lockfile remain unchanged from Phase 1.
- No encryption, secure ACL setup, signatures, tamper evidence or protection against hostile same-account file replacement. Source/actor labels are user assertions. Secret heuristics have false positives/negatives; plaintext exports/backups require deliberate handling.
- Import bounds are 256 MiB input, 100,000 rows and 64 MiB decoded values per table. Import can hold multiple tables in memory; SQLite parsing/integrity checks are not an adversarial resource sandbox. Whole-store backup/restore supports stores outside project-transfer bounds. No automatic divergent-history resolver or corrupt-file salvage exists.
- Older binaries reject schema 3. Existing data migrates safely, but downgrade is not supported; keep a pre-migration snapshot when rollback to an older binary matters.
- Literal retrieval still scans content and provides no semantic ranking or inference of truth. Kind/key filters are exact. Freshness depends on the local clock; invalidation is explicit judgment. Important requirements can be omitted by an explicitly selected filter, so future adapters must define and test their delivery policy.
- Failure injection does not cover every filesystem permission, migration interruption, publication race, physical disk-full or power-loss scenario. Directory flush durability and hostile filesystem races are not guaranteed.
- SQLite bundles are versioned local interoperability data, not an agent API. There is no versioned machine-readable retrieval/error contract, context budgeting/delivery acknowledgment, supported Codex hook or end-to-end adapter test yet. Boundary checks remain observations, not enforcement; model internals/native context remain inaccessible.

The data foundation supports designing a constrained integration next, but **automatic Codex integration is not cleared**. First close advisory/CI gates and establish a machine-readable retrieval contract with acceptance fixtures for requirements, invalid/stale/conflicting knowledge, privacy, timeouts and recovery. Then verify one actual platform integration surface and test supplied context/observable outcomes before enabling automation. No specific Codex capability is assumed here.

Technical basis: SQLite recommends disabling [trusted schemas](https://www.sqlite.org/pragma.html#pragma_trusted_schema); its [defensive configuration](https://www.sqlite.org/c3ref/c_dbconfig_defensive.html) restricts SQL features that deliberately corrupt databases. [Online backup](https://www.sqlite.org/backup.html) underlies snapshots; neither setting establishes tamper resistance or a power-loss guarantee.

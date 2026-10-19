# Phase 10 — Production hardening and security review

Review date: 2026-10-19. Starting local HEAD: `50835a7` (Phase 9), with a clean
working tree. This is a targeted source review and deterministic Windows test
pass, not a security certification or a release. No provider sessions, model
requests, network-dependent tests, benchmarks or soak tests were used.

## Confirmed defects and fixes

Severity describes potential impact under the stated prerequisites; no actual
compromise was observed.

| Issue | Severity and evidence | Fix and regression |
| --- | --- | --- |
| Managed commands selected bare `node` through executable search | High: a project-local `node.exe` could run as the user even for local `codex status`. A harmless invalid executable in the disposable project caused Windows error 216 before the fix, despite a trusted Node directory later on PATH. | `src/codex.rs` now reuses `src/native.rs`'s existing absolute, canonical executable resolver. `scripts/check-everyday.cjs` verifies local status succeeds with the poisoned directory first on PATH. No Codex executable exists in the fixture. |
| Legacy provider discovery accepted an absolute working-directory PATH entry | Medium: requires the working directory or its alias on PATH; a repository provider script could win over the installed provider. The regression selected the repository dummy script before the fix. | `src/provider.rs` canonicalizes PATH directories and skips the current directory, including junction aliases. `tests/provider_launch.rs` verifies both are skipped and the trusted fake provider's output and exit code survive. Explicit overrides and legacy `.cmd`/`.bat` support remain. |
| Windows Codex hook used unqualified `powershell.exe` | Medium: helper selection depended on the provider shell's search behavior and environment, potentially allowing workspace/PATH shadowing. The generated command had no absolute interpreter path; the strengthened assertion failed before the fix. No live-provider exploit was attempted. | `integrations/native/native.cjs` selects the quoted absolute `SystemRoot` Windows PowerShell executable and rejects unavailable/unsafe interpreter paths. The existing encoded-command regression executes the actual adapter with poisoned cwd/PATH. A missing-helper fixture verifies fail-closed setup, owned-directory cleanup and no provider launch. Claude's exec-form hook does not depend on PowerShell. |

The existing native resolver is shared rather than replaced. Managed `--node`
must now designate an existing native executable on Windows; arbitrary scripts
are not Node executable overrides. Ordinary provider arguments, authentication,
UI handoff and explicit context opt-in are unchanged. There are no new commands,
dependencies, database migrations, settings edits or README changes.

## Existing safeguards reviewed and exercised

- SQLite uses parameterized record writes, foreign keys, defensive mode,
  `trusted_schema=OFF`, schema checks and transactional migrations. Existing
  tests cover legacy stores, corrupt/foreign/future stores, concurrent writes,
  failed imports, killed imports, rollback and divergent provenance.
- Backup uses SQLite's snapshot API and publishes a synced temporary file without
  overwriting a destination. Restore requires a new explicit directory; imports
  merge transactionally by origin and reject cross-project or divergent origins.
  No user store was repaired, deleted, reset or migrated by this review.
- Context retrieval uses one read-only transaction. Native validation retains
  protocol/schema checks, canonical project isolation, nested-project boundaries,
  mandatory coverage, freshness, conflicts, provenance, duplicate-key rejection,
  sensitive-value heuristics, fatal UTF-8 decoding and byte limits.
- Every supported `SessionStart` source (`startup`, `resume`, `clear`, `compact`,
  `fork`) still retrieves current context. Identical fingerprints do not suppress
  recovery. Bounded metadata records preparation/output stages, not conversations;
  output success is not a provider receipt or proof of adherence.
- Native fixtures cover invalid events, timeouts, broken output, interruption,
  unsupported probes, configuration sentinels, bounded receipts and redirected
  cleanup. No provider trust bypass or automatic restart was added.
- Installer/package review retained exact archive entry names, size limits,
  outer and per-file hashes, staged installation into a new directory, canonical
  parent checks, redirected-cleanup rejection and original-error preservation.
  The adapter remains embedded in Rust and included in the complete release
  source inventory checked by `validate-build-info.cjs`.

## Verification

The reproducible gate is:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/verify.ps1 -DeterministicOnly
```

It uses cached dependencies with `--locked --offline`, formatting, Clippy with
warnings denied, Rust tests, a release build, dependency-notice checks, schema,
context, session, everyday/native integration, package/installer and PE/PATH
checks. Its pilot-harness step tests only pure offline evaluator fixtures; it
does not invoke the pilot or any model. Optional model, pilot, benchmark and soak
modes are excluded. The existing ignored performance test remains ignored.

Final results and source/log hashes are recorded in
[the Phase 10 evidence](evidence/phase10-hardening-offline.json).
The final gate exited **0**: formatting, Clippy, debug/release builds and **55 Rust
tests** passed; one existing performance test stayed ignored. Supplemental
checks passed: schema 10, context 23, session 23, everyday 28, native 33,
package/installer 19, PE/PATH 4 and offline evaluator fixtures 12 (**152 total**).
Dependency notices matched all 49 locked Windows packages. Release-profile Rust
tests and static-CRT reproducibility builds were not repeated in this phase.
Temporary package fixtures verify local packaging and installation. They do not
constitute a refreshed distributable or a newly certified portable build.

## Remaining limitations and release gates

- This review identified no remaining critical/high defect requiring a blocker
  within its examined scope. It does not establish absence of vulnerabilities.
- SQLite stores, backups and exports are plaintext. Secret detection is heuristic;
  explicit sharing exposes selected memory to the chosen provider. Provenance is
  self-reported, not tamper-proof. Protect policies evaluate boundaries and do not
  prevent file modification.
- Executable overrides, non-workspace PATH directories, `SystemRoot` and account
  permissions remain trust inputs. This is not a sandbox against an attacker
  already able to modify trusted executables, environment or same-account files.
  Windows temporary-file privacy depends on inherited account ACLs; no new ACL
  isolation or race-free filesystem authorization is claimed.
- Cleanup can fail when files are locked or processes are forcibly terminated.
  Existing diagnostics report this; orphaned temporary metadata/code may require
  manual inspection. No background cleanup or silent data reset was introduced.
- Native provider admission rules and version caveats remain those in Phase 8
  and 8.5. No exact provider version was newly certified. Real configuration
  loading, hook trust, lifecycle scheduling, automatic compaction callbacks,
  receipt and model adherence remain unverified by this phase. The PowerShell
  fixture tests the helper and adapter, not a real provider's command parser.
- Locked dependencies and notices were checked locally, but no online advisory
  database or external security review was consulted. Current vulnerability
  advisory review remains a release task when authorized network access is allowed.
- New source hashes invalidate earlier build receipts. Before public release,
  rebuild the portable candidate with the established reproducible/static-CRT
  procedure, review its new inventory and hashes, run hosted and clean-machine
  acceptance, and decide signing/authenticated provenance. Those external checks
  and any publication require separate authorization; none is implied here.

Ready for Phase 11 release preparation with the recorded offline gate passing;
not approval to publish. All changes remain local for review.

## Changed files

- `src/provider.rs`: canonical legacy provider discovery.
- `src/native.rs`, `src/codex.rs`: reuse safe native Node discovery for managed commands.
- `integrations/native/native.cjs`: absolute Windows hook interpreter and independent Claude setup.
- `tests/provider_launch.rs`: absolute cwd/junction regression.
- `scripts/check-native.cjs`: poisoned helper search and missing-helper cleanup checks.
- `scripts/check-everyday.cjs`: managed local-status executable search regression.
- This report and `docs/evidence/phase10-hardening-offline.json`: review scope,
  evidence, compatibility boundaries and remaining release gates.

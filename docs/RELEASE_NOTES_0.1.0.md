# SwapCtx v0.1.0-preview — release notes

Published on 2026-10-23 as the first public **Windows x64 prerelease**.
[Download v0.1.0-preview](https://github.com/rehnx/SwapCtx/releases/tag/v0.1.0-preview).
CLI/package version: `0.1.0-preview`. The published source and assets remain fixed
when documentation on main changes.

SwapCtx preserves deliberately saved project knowledge across coding sessions.
This first Windows preview combines a Rust CLI, local SQLite memory, optional
Codex/Claude lifecycle adapters and developer-controlled handoffs.

## Included workflows

- Project-scoped requirements, decisions and notes with provenance, history,
  review/expiry, archival, invalidation and supersession.
- Versioned, bounded context retrieval and explicit mandatory-coverage reporting.
- Permanent project intent, read-only memory health and exact duplicate diagnostics.
- Local Git inspection, context explanations and unsigned state fingerprints.
- Reviewed JSON handoffs with transactional conflict handling and optional decisions,
  notes and boundary rules; consistent preview/apply validation.
- Backup/restore, legacy migrations and portable Windows installation checks.

## Reliability corrections in this preview

Writable connections tolerate transient SQLite contention with a bounded
30-second busy wait per SQLite operation; read-only connections retain five
seconds for context-callback responsiveness. This does not replay committed
commands or impose a whole-command deadline. Sustained locks still report errors.

Handoff preview/apply share strict field validation, normalized intent kinds,
duplicate-member rejection and consistent counts. Repeated agent transfers keep
bounded original provenance; unknown provenance remains excluded from mandatory
context. Memory audits use one consistent read snapshot, diagnose exact duplicates
without merging records, and exclude invalidated records from freshness failures.

The Windows producer compares complete binaries from fresh directories, checks
pre/post-native-test identity, rejects dynamic CRT imports and validates the
exact packaged executable. It records selected native compiler/linker/SDK
identities and retains raw differences for private investigation. It does not
normalize binary fields to obtain matching hashes.

## Requirements and installation

Core commands use the Windows x64 executable without Node or Rust. Optional adapters
need Node.js 24+ and separately installed/authenticated providers. Follow the
[Windows installation guide](WINDOWS_RELEASE.md) and verify the installer and ZIP
against hashes supplied through a trusted channel. Checksums detect changed bytes;
they do not authenticate a publisher. No WinGet or Scoop package is submitted.

## Known limits

Memory, backups and handoffs are plaintext. Secret detection is heuristic; provenance
is self-reported. Context preparation and callback output do not establish provider
receipt or model adherence. Native providers are tested with synthetic fixtures;
exact live compatibility remains unverified. Boundary rules are manual checks and
do not prevent writes. State fingerprints are unsigned snapshots with filesystem
race limitations. No telemetry or automatic transcript ingestion is introduced.
Windows hosted fixtures do not establish independent-machine compatibility.
Same-toolchain matching builds do not establish byte reproducibility across
different native toolchains, signed provenance or publisher identity. The preview
is unsigned and not a security-certified product. Linux CI checks core contracts;
no Linux distribution or support guarantee is included in this Windows preview.

## Published identity and evidence

- Source: [`962e1e649851684fab897b96f714e493f1e1c969`](https://github.com/rehnx/SwapCtx/commit/962e1e649851684fab897b96f714e493f1e1c969),
  tag `v0.1.0-preview`.
- [Final hosted CI](https://github.com/rehnx/SwapCtx/actions/runs/39902646906):
  five applicable jobs passed; public-repository `windows-recipient` was skipped.
- Published ZIP, installer, executable hashes, and independently recomputed
  download checks are in [release verification](RELEASE_VERIFICATION.md).
- [The owner reports a separate-machine recipient pass](https://github.com/rehnx/SwapCtx/issues/6#issuecomment-6358810928)
  for these final assets. The referenced sanitized JSON was not attached or
  linked at review; detailed machine/check results remain unreviewed.
- The preview is unsigned. Private reporting instructions are in
  [SECURITY.md](../SECURITY.md).

The [launch decision record](LAUNCH_READINESS.md) and
[owner checklist](OWNER_LAUNCH_CHECKLIST.md) preserve prerelease preparation history;
their pending-publication language is not the current download status.

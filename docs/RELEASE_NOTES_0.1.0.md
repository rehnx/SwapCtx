# SwapCtx v0.1.0-preview — release notes (unpublished)

Unpublished prerelease text. No release or tag is authorized by this document.
Append the final release packet's exact source, CI and tested-asset identity block
before publication. Never reuse an earlier candidate's hashes or rebuild approved
assets. The CLI/package version and proposed tag are `0.1.0-preview` and
`v0.1.0-preview`.

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

## Candidate identity — must be completed before publication

- Approved source SHA: use the exact release packet; proposed tag `v0.1.0-preview`.
- Final hosted checks and fresh Windows producer run: record the exact run URL.
- Tested executable, ZIP and installer SHA-256: extract from that run's evidence.
- Independent Windows recipient acceptance: pending.
- Signing/unsigned-preview trust decision and verified private security intake: pending.
- Founder authorization for visibility, publication and distribution: pending.

See [launch readiness](LAUNCH_READINESS.md). Public-launch status remains NO-GO
until the required evidence and authorizations are complete.
Owner-only steps and the independent acceptance procedure are in the
[launch checklist](OWNER_LAUNCH_CHECKLIST.md).

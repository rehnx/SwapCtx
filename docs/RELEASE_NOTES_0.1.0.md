# SwapCtx 0.1.0 preview — draft release notes

Unpublished preparation. No release or tag is authorized by this document. Before
publication, replace the candidate identity section with the final approved source,
CI evidence and exact tested asset hashes. Never reuse an earlier candidate's hashes.

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

## Candidate identity — must be completed before publication

- Approved source SHA and tag: pending; proposed tag name `v0.1.0`.
- Final hosted checks and fresh Windows producer run: record the exact run URL.
- Tested executable, ZIP and installer SHA-256: extract from that run's evidence.
- Independent Windows recipient acceptance: pending.
- Signing/unsigned-preview trust decision and verified private security intake: pending.
- Founder authorization for visibility, publication and distribution: pending.

See [launch readiness](LAUNCH_READINESS.md). Public-launch status remains NO-GO
until the required evidence and authorizations are complete.

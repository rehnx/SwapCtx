# SwapCtx documentation

Start with the [README quick start](../README.md#quick-start) or download the
[public Windows x64 preview](https://github.com/rehnx/SwapCtx/releases/tag/v0.1.0-preview).
SwapCtx provides external project memory. It does not enlarge model context
windows, guarantee adherence, or intercept file writes.

## Getting started

- [Install, verify downloads, upgrade, and uninstall on Windows](WINDOWS_RELEASE.md)
- [CLI usage: projects, memory, context, policies, and recovery](USAGE.md)
- [Troubleshooting and safe bug reports](../SUPPORT.md)
- [Published release notes](RELEASE_NOTES_0.1.0.md)
- [Published asset integrity and acceptance evidence](RELEASE_VERIFICATION.md)

## Daily memory workflows

- [Project intent, memory health, explanations, worktree review, and handoffs](INTELLIGENCE_WORKFLOWS.md)
- [Memory lifecycle and project isolation](USAGE.md#memory-lifecycle)
- [Memory health diagnostics and readiness states](MEMORY_QUALITY.md)
- [Backup, project transfer, and restore](USAGE.md#backup-transfer-and-restore)
- [Privacy, plaintext storage, and private vulnerability reporting](../SECURITY.md)

Core commands work without an AI provider. Context is project-scoped; use the same
data directory consistently. Backups, exports, and handoffs can contain sensitive
plaintext, including history.

## Optional provider integrations

- [Native Codex and Claude Code setup, probes, trust, and limitations](PHASE8_NATIVE_PROVIDERS.md)
- [Native stabilization and local callback diagnostics](NATIVE_INTEGRATION_STABILIZATION.md)
- [Managed Codex console setup and observatory](PHASE5_EVERYDAY_CODEX.md)
- [Outstanding live-provider compatibility work](https://github.com/rehnx/SwapCtx/issues/7)

Native sharing is experimental and opt-in. Synthetic adapter tests and local
output do not establish provider receipt or model adherence. The managed console
is a separate workflow; historical results from it do not certify native hooks.

## Architecture and contracts

- [Data flow](../README.md#how-memory-works) and [source map](../CONTRIBUTING.md#repository-map)
- [Context protocol v1](CONTEXT_PROTOCOL.md) and [JSON Schema](schema/context-v1.schema.json)
- [Project scoping, storage selection, and legacy recovery](USAGE.md#projects-and-storage)
- [Intent, inspection, verification, and handoff contracts](INTELLIGENCE_WORKFLOWS.md)
- [Unified integration architecture and engineering decisions](UNIFIED_INTEGRATION.md)
- [Windows dependency inventory](DEPENDENCIES.md) and [third-party notices](../THIRD_PARTY_NOTICES.txt)

The core is a Windows-first Rust CLI backed by local SQLite. Node.js adapters
bridge optional provider interfaces. There is no storage/retrieval network client,
daemon, or automatic transcript ingestion. Contract and engineering documents
describe their stated revision; consult release evidence for the shipped binary.

## Contributing

- [First contribution, development setup, and verification](../CONTRIBUTING.md)
- [Current priorities and open work](ROADMAP.md)
- [Community conduct](../CODE_OF_CONDUCT.md)
- [Bug and feature templates](https://github.com/rehnx/SwapCtx/issues/new/choose)

## Release engineering and historical evidence

These documents preserve milestone decisions, tests, and limitations at the time
they were written. References to unpublished candidates, pending launch approval,
or earlier hashes are historical, not the current download status. The published
`v0.1.0-preview` tag and assets remain fixed when documentation on main changes.

- **Producer and distribution:** [release preparation](RELEASE_PREPARATION.md),
  [portable packaging](WINDOWS_RELEASE_ENGINEERING.md),
  [distribution hardening](PREVIEW_DISTRIBUTION.md),
  [build readiness](RELEASE_BUILD_READINESS.md), and
  [binary comparison investigation](WINDOWS_REPRODUCIBILITY.md).
- **Launch records:** [decision record](LAUNCH_READINESS.md),
  [owner checklist](OWNER_LAUNCH_CHECKLIST.md),
  [Phase 11 readiness](PHASE11_RELEASE_READINESS.md),
  [Phase 10 hardening](PHASE10_PRODUCTION_HARDENING.md),
  [Phase 6 handoff](PHASE6_RELEASE_APPROVAL.md), and
  [Windows CI investigation](PHASE7B_WINDOWS_CI_FIX.md).
- **Native integration:** [selector design](NATIVE_PROVIDER_LAUNCHER.md),
  [Phase 8.5 verification](PHASE8_5_NATIVE_VERIFICATION.md),
  [Phase 9 continuity](PHASE9_CONTEXT_CONTINUITY.md),
  [quality review](NATIVE_INTEGRATION_QUALITY_REVIEW.md),
  [Windows hook failure](CODEX_WINDOWS_HOOK_FAILURE.md), and
  [hook delivery investigation](CODEX_HOOK_DELIVERY_INVESTIGATION.md).
- **Earlier core and managed-console milestones:** [foundation audit](FOUNDATION_AUDIT.md),
  [Phase 2 reliability](PHASE2_RELIABILITY.md), [Codex readiness](CODEX_READINESS.md),
  [Phase 3 lifecycle](PHASE3_CODEX_LIFECYCLE.md),
  [Phase 4 long sessions](PHASE4_LONG_SESSIONS.md),
  [managed-console pilot](REAL_WORLD_CODEX_PILOT.md), and
  [retrieval/session reliability](RETRIEVAL_SESSION_RELIABILITY.md).

Machine-readable milestone records remain in `docs/evidence/`. Test counts,
authenticated pilot results, and hashes apply only to their recorded revisions
and environments. Hosted Windows tests do not certify a clean personal machine;
Linux checks do not establish Windows behavior.

# Changelog

This file tracks user-facing changes. Entries below are **unreleased**; the
`0.1.0` Cargo version does not establish a published release or tag.

## Unified integration candidate

- Integrate native CLI presentation and stabilized Codex hooks with intent,
  memory audit, inspection, explainability, JSON handoffs and SHA-256 verification.
- Scope Git evidence to nested registered projects; check alias destinations and
  both staged/current contents, reject lossy paths, and disable diff helpers.
- Redact sensitive JSON explanations, bound and atomically publish handoffs,
  retain mandatory imported intent and support explicit decision/note transfer.
- Preserve legacy read/migration behavior, refresh dependency notices, and add
  Linux native fixtures plus a fresh offline Windows preview CI gate.
- Candidate evidence and remaining limitations: [integration report](docs/UNIFIED_INTEGRATION.md).

## Unreleased — Windows preview preparation

### Presentation and release preparation

- Reorganized the README around installation, explicit memory use and native
  provider workflows, with a documented architecture and compatibility limits.
- Added contributor, security, support and issue/PR guidance.
- Added current release instructions and a separately recorded local candidate
  assessment. Historical engineering evidence remains available.
- Fixed fresh release builds in deeply nested Windows checkouts by using shorter
  owned compile staging, while preserving destination-adjacent package assembly
  and originating failures during cleanup.

### Existing implementation included in this candidate

- Local project-scoped SQLite memory, lifecycle/provenance history, conflict and
  expiry handling, versioned context retrieval and manual boundary policies.
- Whole-store backup/restore and project-scoped export/import with integrity checks.
- Original Codex and Claude Code terminal launchers and explicit opt-in native
  lifecycle adapters. Repeated recovery callbacks retrieve fresh validated memory;
  bounded diagnostics distinguish preparation and output from unknown receipt.
- Preserved managed Codex console and its optional local observatory.
- Hardened executable discovery against working-directory shadowing, including
  canonical PATH/junction handling and an absolute Windows hook interpreter.
- Portable Windows packaging, offline hash-verifying installation, dependency
  notices, static-runtime build comparison and recipient acceptance tooling.

These summarize work already in the source, not new Phase 11 application features.
See [Phase 8](docs/PHASE8_NATIVE_PROVIDERS.md),
[8.5](docs/PHASE8_5_NATIVE_VERIFICATION.md),
[9](docs/PHASE9_CONTEXT_CONTINUITY.md) and
[10](docs/PHASE10_PRODUCTION_HARDENING.md) for detailed changes and evidence.

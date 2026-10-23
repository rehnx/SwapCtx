# Roadmap and contribution priorities

[SwapCtx v0.1.0-preview](https://github.com/rehnx/SwapCtx/releases/tag/v0.1.0-preview)
is publicly available for Windows x64. The published source tag and assets remain
fixed. Current work should improve usability and evidence while preserving local
SQLite memory, project isolation, explicit sharing, and recoverability.

This page describes areas for discussion, not delivery commitments or a schedule.
Check the [open issues](https://github.com/rehnx/SwapCtx/issues) before starting work.

## Current follow-ups

- [Independent Windows acceptance evidence (#6)](https://github.com/rehnx/SwapCtx/issues/6):
  the owner reports a pass for the final package on a separate Windows machine.
  A publicly accessible sanitized JSON report is still needed; see
  [release verification](RELEASE_VERIFICATION.md).
- [Native provider compatibility (#7)](https://github.com/rehnx/SwapCtx/issues/7):
  record exact provider versions and distinguish hook loading, fresh local output,
  receipt, and adherence in separately authorized evaluations. Until then, native
  sharing remains experimental.
- [Handoff expiry guidance (#10)](https://github.com/rehnx/SwapCtx/issues/10):
  explain why an exact imported match retains local expiry and can still need
  explicit review. Do not renew lifecycle metadata silently.
- [Private intake and publisher trust (#5)](https://github.com/rehnx/SwapCtx/issues/5):
  reconcile the historical gate with the published unsigned preview and the
  documented private reporting route. This issue remains open; its older comments
  are not current publication status.

The [Windows build comparison investigation (#13)](https://github.com/rehnx/SwapCtx/issues/13)
is closed within its recorded scope. Same-toolchain matching builds do not establish
byte reproducibility across different native toolchains or hosted images.

## Useful contributions

- Clear installation, PATH, recovery, and memory-lifecycle documentation.
- Focused diagnostic improvements backed by disposable synthetic reproductions.
- Deterministic retrieval, project-scope, and fake-provider regression cases.
- Better explanations of context coverage, exclusions, and readiness states.

Discuss additional providers, platform distributions, performance work, and larger
workflow changes before implementation. No new package-manager availability,
platform support, or provider certification is promised here.

Start with [contributing](../CONTRIBUTING.md) and the [documentation index](README.md).
Historical launch decisions remain in [launch readiness](LAUNCH_READINESS.md) and
the [owner checklist](OWNER_LAUNCH_CHECKLIST.md); they do not supersede the public release.

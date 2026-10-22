# Native integration stabilization

Date: 2026-10-20. Original repository HEAD: `b84b3cd`, with earlier native fixes
and uncommitted UI/publication work preserved. No other branch/worktree was
modified. This sprint ran no installed provider, help/version probe, authenticated
session, model/API request, benchmark or soak test.

## Observed Codex milestone

The user reported a later authorized Codex run with these local observations:

| Observation | Reported value |
| --- | --- |
| Windows bootstrap / Node entry / config validated | true / true / true |
| Receipt write failed | false |
| Contexts prepared / local output writes completed | 1 / 1 |
| Failure observations / unfinished callbacks | 0 / 0 |

This establishes reported native hook execution, validated local preparation and
completed local stdout output. The model reportedly returned a SWAP-formatted
marker, but its complete value was not retained for independent comparison.
**Exact adherence remains unconfirmed.** No raw provider acknowledgement, full
marker, transcript or credential was supplied or stored by this sprint. These are
user-supplied observations, not a new agent-run reproduction or a guarantee of
provider acceptance, retained memory, compaction recovery or future adherence.
The previously reviewed environment was Codex 0.163.0; no installed version was
re-probed. The earlier failed runs remain documented in the
[shell report](CODEX_WINDOWS_HOOK_FAILURE.md) and
[delivery investigation](CODEX_HOOK_DELIVERY_INVESTIGATION.md).

## Recurring trust: stable definition, disposable session state

The [official hooks guide](https://learn.chatgpt.com/docs/hooks) requires review
of a non-managed hook's exact definition; changed definitions need review again.
The pinned [0.163.0 discovery source](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/hooks/src/engine/discovery.rs)
hashes the normalized event/matcher/handler and applies enabled/trusted/managed
policy gates. Random command paths therefore caused needless definition changes.
The [command runner](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/hooks/src/engine/command_runner.rs)
replays its captured session environment to hooks. The pinned
[environment exclusions](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/protocol/src/shell_environment.rs)
do not exclude SwapCtx's two internal path variables. These are source-level
compatibility grounds, not proof of trust persistence in the installed UI.

Codex now receives the same definition for unchanged adapter bytes, Node and
system-interpreter paths, bootstrap encoding and project/store/SwapCtx executable
scope. Its command reads `SWAPCTX_NATIVE_SCRIPT` and `SWAPCTX_NATIVE_CONFIG` from
the **provider child's** environment. Only temporary paths travel there; no memory
or conversation is placed in the environment. Inherited case-insensitive aliases
are removed before these per-launch values are supplied. The parent environment,
provider auth/configuration and trust records are never edited.

The reviewed command pins SHA-256 of the complete adapter source plus a
domain-separated scope identity containing canonical provider/project/store/
SwapCtx executable paths. The callback reads bounded regular-file bytes, rejects
redirected final script entries, verifies the digest and compiles those exact
verified bytes with Node's public `vm.compileFunction` API. It does not check a
file then re-open it for execution. Imports are restricted to built-in modules.
This is trusted local adapter execution, not a JavaScript security sandbox or
protection against a malicious same-user process.

Configuration must match the pinned canonical scope before retrieval. All existing
packet, freshness, conflict, provenance, sensitive-value, mandatory coverage,
UTF-8, duplicate-key and byte-budget checks remain. Every callback reads current
memory; no context cache or permanent delivery skip was introduced.

Only the fixed bootstrap is compressed/encoded into the command to keep Windows
cmd dispatch below its length limit. Commands above 8,000 characters fail safely
with `windows_hook_command_too_long`. Adapter changes alter the reviewed definition;
new project/store/executable scopes and changed runtime paths require new review.
Memory edits do not change hook identity: subsequent callbacks still validate fresh
snapshots. Runtime/encoding changes may also change the command and require review.

No stable script cache, persistent registration, global settings, project
instruction file, approval bypass or trust auto-write was added. Concurrent
launches retain separate random configuration, diagnostic/HMAC keys and receipt
files. Normal cleanup removes them. Old callbacks cannot reuse removed configs,
and another project's environment/config is rejected rather than routed to its
memory. Provider trust remains required, including after upgrades and where
administrative policy disables or restricts hooks.

**Offline definition stability is verified; actual persisted Codex trust reuse is
not.** Changed definitions must be reviewed normally in `/hooks`. A provider that
does not preserve these environment variables fails closed with an actionable
bootstrap code rather than reverting to stale or alternate context. The existing
admission probe still detects recognizable version/help capabilities, not every
release's complete contract. The local `--check` reports stable identity preparation
without probing a provider or claiming execution. Claude retains its existing
temporary session-only plugin; no real Claude hook compatibility is established.

## Failure handling and everyday workflow

The original terminal UI, arguments, exit handling and explicit memory opt-in
remain. Use the same global storage flags and registered project as usual:

```powershell
swapctx
swapctx native codex --check
swapctx native codex --context
swapctx native codex --context -- resume SESSION_ID
swapctx native claude --check
swapctx native claude --context
```

The first command retains the native provider selector and consent prompt.
Plain `swapctx native codex`/`claude` and legacy `swapctx launch codex`/`claude`
do not share memory. The existing managed console remains separate and unchanged.
Inside an authorized Codex session, review new/changed definitions in `/hooks`.
Do not infer trust reuse from matching command text or a successful health check.
If callbacks are absent, capture installed/active/trust status and hook errors
before submitting a model prompt. A bare `/clear` may request a local lifecycle
refresh after review in a disposable session; do not force compaction to diagnose
loading. Model-based adherence checks remain separately authorized work.

New safe failures are `hook_session_environment_invalid`,
`hook_adapter_integrity_failed`, `hook_adapter_load_failed`,
`hook_session_scope_mismatch` and `windows_hook_command_too_long`.
Missing adapter, output failure and existing context errors remain specific.
No error includes memory or raw exception/path text. Stage flags can still be
absent/incomplete after early failure; they are best effort, not provider receipt.
Permission/race failures and forced termination can leave temporary metadata,
which the existing cleanup warning reports without deleting outside owned paths.

Input remains bounded to 32 KiB/3 seconds, retrieval to 5 seconds, the provider
handler to 10 seconds, and final context to 8,192 UTF-8 bytes. Startup overhead
fits focused fixtures, but no performance benchmark or provider latency claim is
made. No timeout was relaxed, provider restarted or user policy overridden.

## Offline verification

Focused development checks passed for Windows outer shells, stable definitions,
adapter integrity/scope/environment failure cases, child-only environment handling,
and the actual Rust CLI/embedded adapter/fake-provider lifecycle chain for both
providers. Negative tests confirm that modified source does not execute and that
missing/expired/other-project state supplies no context. Existing privacy and
interruption/cleanup tests remain in the complete native suite.

The complete deterministic gate completed once after implementation, with exit 0:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -DeterministicOnly
```

Formatting, Clippy, the locked offline release build, 55 Rust tests (one existing
performance test ignored), 165 supplemental checks including all 46 native tests,
49-package notice inventory and the existing documentation gate passed. The
expected input-timeout diagnostic came from a negative fixture. New report links
and fences were checked separately; saved UI/console/publication/plan file hashes
still matched their starting values. No source/UI formatting changes were needed.

Final results and local log hashes are recorded in
[stabilization evidence](evidence/native-integration-stabilization.json).
That evidence distinguishes this offline run from the user-reported live milestone.
No public artifact is prepared or published by this sprint.

## Dedicated Git checkpoint plan (not staged)

Review `git diff` for each tracked file and the complete contents of each new file.
The original index is left untouched. Suggested native-only staging set:

| File | Reason to include |
| --- | --- |
| `integrations/native/native.cjs` | Prior shell/stage fixes plus stable, pinned, scoped command and child environment |
| `scripts/check-native.cjs` | Prior shell/stage regressions plus stability/security regressions |
| `tests/fixtures/native-provider.cjs` | Actual cmd dispatch and launch-local synthetic provider environment |
| `tests/fixtures/windows-hook-runner.rs` | Required earlier Windows Rust shell-dispatch regression fixture |
| `docs/CODEX_WINDOWS_HOOK_FAILURE.md` | Historical first-failure evidence and limitations |
| `docs/CODEX_HOOK_DELIVERY_INVESTIGATION.md` | Historical diagnostic investigation and no-prompt procedure |
| `docs/PHASE8_NATIVE_PROVIDERS.md` | Current trust/lifecycle guidance |
| `docs/NATIVE_INTEGRATION_STABILIZATION.md` | This milestone, implementation, limits and staging plan |
| `docs/evidence/native-integration-stabilization.json` | Offline results and clearly attributed live observations |

Use explicit file paths for any future approved staging; never `git add .`.
Do **not** stage `src/native.rs`, `src/main.rs`, `src/provider.rs`, `src/display.rs`
or `integrations/codex/session.cjs`: their current differences are Gemini's CLI/
console work, with no new sprint changes. Leave README, CHANGELOG, community/
security/support and release docs, `docs/FINAL_PREPUBLICATION_REVIEW.md`,
`docs/NATIVE_PROVIDER_REALITY_CHECK_PLAN.md` and `tatus --short` intact and unstaged.
The historical reality-check plan can be reviewed in a separate documentation
checkpoint; do not silently discard or rewrite it. No commit/push/Git history
operation or Antigravity worktree operation is authorized here.

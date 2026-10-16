# Phase 4: long-session reliability and automatic recovery

Windows implementation and controlled verification, 2026-10-17. Rust 1.99.0, Node 24.17.0, Codex CLI 0.162.1. Storage schema, locked dependencies, default read-only console mode and context protocol v1 are unchanged. **Extended real-model comparison and natural automatic recovery remain unverified.**

## Evidence accepted and limits

The locally supplied [successful authenticated pilot report](evidence/codex-pilot-2026-10-17-passed.json) was checked and preserved from `.build/codex-pilot-20261017-025756.json` without overwriting earlier evidence. It records all eight checks passed: the real-provider context-use control, six coding stages, resumption, native manual compaction and adherence to the superseding requirement. All seven operations retain one thread ID; the six coding operations record the unchanged contract-file hash. This is the user's completed run outside the restricted tool process; it was inspected here, not independently replayed here. An earlier local report ended with `pilot_interrupted`; that partial run is not evidence of a model adherence failure.

The successful report explicitly leaves `automaticCompactionVerified` and `longRunningSessionVerified` false. It does not record token counts, CPU or RSS. Across its seven operations, context-read p50/p95 was **49/74 ms**, session preparation **1,670/6,282 ms**, and total operation time **33,921/38,785 ms**. Selected packets were 2,252–2,265 bytes. Those are prior-build observations on a small workload, without a baseline; they establish neither a speedup nor a token saving.

This execution process still cannot run new real-provider trials. A fresh diagnostic recognizes the login with explicit `CODEX_HOME`, but app-server initialization fails with `codex_startup_permission_denied`; unauthenticated probes to both OpenAI hosts return `EACCES`. The [Phase 4 attempt](evidence/codex-soak-2026-10-17-attempt.json) stops before model evaluation, records empty arms and null comparison measurements, and confirms disposable-directory cleanup. Earlier [environment evidence](evidence/codex-environment-2026-10-17-user-home.json) records the same restrictions. This does not invalidate the user's successful ordinary-terminal pilot or establish a problem with that terminal's connectivity.

## Changes implemented

Memory-only refreshes now append the current external-memory block without repeating unchanged user developer instructions. If the user's developer policy changes, the combined current policy and memory block is appended. Native runtime instructions still contain both so compaction can reconstruct them. Unchanged context appends no additional snapshot. Controlled native requests verify these cases. Earlier snapshots may remain in history; current requirements are not guaranteed to override model behavior.

The stdio reader now uses geometric buffer growth for fragmented frames and parses complete frames directly when possible. This removes repeated whole-prefix copying while keeping an 8 MiB partial-frame limit. Valid coalesced frames can exceed that limit in aggregate. Tests cover fragmented Unicode, oversized fragmented frames and a multi-frame batch over 8 MiB. This is an algorithmic reduction in copying; no end-to-end real-provider CPU or latency improvement has been measured.

Additive `swapctx.codex` version-1 observability:

| Event | Added evidence |
| --- | --- |
| `token_usage` | Validated safe nonnegative integer `total` and `last` counters, optional cache-write counts, and the reported model context-window metadata |
| `warning` | `invalid_token_usage` when counters cannot be represented safely; `model_rerouted` when the native runtime reports rerouting |
| `operation_metrics` | Adapter-process CPU user/system milliseconds, RSS and heap snapshots, alongside existing monotonic phase timings |
| `context_selected` | Explicit history-injection bytes and runtime-instruction bytes, alongside packet bytes/hash |
| `session_ready` | Configured model identifier when representable |

CPU/RSS measurements describe the Node adapter process only. They exclude the Rust retrieval child, Codex process, tools and provider; RSS is a snapshot, not a peak. Packet/instruction bytes are not token counts or the model's whole context. Model context-window metadata is a native report, not hidden-state inspection or native context expansion. Usage counters do not establish billing; cached/reasoning components must not be added to totals again. No telemetry is sent elsewhere by SwapCtx.

## Paired longer-session evaluator

`scripts/check-soak.cjs` compares the managed engine with a plain native app-server lifecycle using the same per-turn fresh-process strategy, sandbox request, configured user developer instructions and equivalent disposable workspaces. It exercises the actual managed implementation and real CLI retrieval, not the interactive stdin frontend; that frontend has the earlier pilot evidence. Both arms receive identical initial requirements and explicit revisions every six turns. The baseline receives those facts in ordinary user messages and never reads SwapCtx or receives a tagged packet. Facts are not withheld to manufacture a benefit.

The default is 24 turns per arm with a 120-minute total deadline. Twelve API implementation steps are followed by maintenance, edge-case review and refactoring tasks in a small CommonJS text-library fixture. Every revision uses a fresh release identifier. Both workspaces include a frozen historical archive containing an explicitly obsolete identifier and a frozen case fixture. Only `lib.cjs` may change. Independent executable checks evaluate current requirements, all earlier APIs and frozen files after every turn. These are test-workload APIs, not new SwapCtx product features.

Arm order alternates by round, with a randomly selected first arm recorded in the report; `--first managed|baseline` permits counterbalanced repeats. The evaluator requests no manual compaction, changes no context threshold, fabricates no usage and inserts no arbitrary context padding. Natural automatic compaction may therefore not occur within the run budget. A single library trial is not representative of every repository, coding task or session duration.

The evaluator observes native compaction notifications and requires successful artifact checks both in an automatically compacted managed turn and in a later turn before marking `automaticRecoveryObserved`. That is observed behavior for those turns, not proof that every requirement was lost then restored internally. Changes to saved memory during an in-flight turn remain deferred to the next turn, as before. It does not inspect hidden model state or intercept file operations.

Summaries include behavior pass differences, timing p50/p95, local adapter resource snapshots, injection bytes and final native cumulative token counters. Cumulative snapshots are never summed. Missing, unsafe or decreasing/reset counters yield unknown totals, not zero-cost estimates or reconstructed billing. Incomplete pairs, different configured models or reported rerouting prevent comparison claims. A single paired run never sets `causalImprovementEstablished`; repeat trials are needed to assess variability and generality. Different compaction schedules and unrecorded provider/runtime differences can still confound comparisons.

The report is claimed without overwriting an existing file and checkpointed after each completed arm. Updates require the same run ID and refuse changed ownership or unsafe file types. This is cooperative file protection, not a tamper-proof audit. A killed runner can leave a `running` partial report; it is not a pass and the runner does not resume that evaluation automatically. Normal completion/failure cleans only its own disposable root. Native Codex can retain its own probe history; SwapCtx does not delete user history. Reports omit raw transcripts, commands, credentials, saved contents and working source, retaining safe metadata and hashes. The independent oracle uses the same restricted child/environment as the pilot and is not a complete sandbox against hostile generated code.

The controlled baseline test exposed a native response granting read-only despite a workspace-write request in this fixture. Adding a legacy config override did not resolve it, and the cause is not established. Both implementations retain strict sandbox validation; the real evaluator refuses a mismatch instead of continuing under another policy. Native baseline start/resume, usage collection and absence of SwapCtx packets are separately tested under explicitly requested read-only mode. That is transport evidence, not workspace editing evidence. No permission bypass or user-config rewrite was added.

## Run outside the restricted tool process

Use the authenticated Windows terminal that completed the pilot, with native Codex home/cache/history access and provider connectivity:

```powershell
$Codex = 'C:\absolute\path\to\codex.exe' # version 0.162.1
$SwapCtx = (Resolve-Path 'target\release\swapctx.exe').Path
New-Item -ItemType Directory -Path .build -Force | Out-Null
$Report = Join-Path $PWD '.build\soak-NEW.json'
node scripts/diagnose-codex.cjs $Codex --network
node scripts/check-soak.cjs $Codex $SwapCtx $Report --turns 24 --minutes 120
```

All three paths supplied to the soak runner must be absolute; the report's parent must exist and the file must be new. Supported turn budgets are 6–96, deadlines 1–720 minutes. A completed default trial requests 48 coding turns plus any native automatic-compaction/retry operations, using the configured provider and billing. Use a new report for each repeat, alternate `--first`, and keep configuration stable. The runner does not choose a replacement model or copy credentials. A low turn budget is useful for smoke tests but may not exercise automatic compaction.

Exit codes: **0** means complete pairs, all managed behavior checks passed, observed automatic recovery and complete scoped performance counters; **1** means a failed operation/check prerequisite; **2** means a recognized external prerequisite is blocked; **3** means incomplete coverage, time budget, missing comparable measurements or no automatic-recovery observation. A report with all ordinary coding checks passing can still be incomplete. An observed zero or negative benefit must be retained as such.

## Verification and readiness

Windows verification passed formatting, Clippy with warnings denied, release build and the 47 native tests in debug and release. All 102 supplemental checks passed: 10 SQL, 19 context/legacy-adapter, 23 session/observability, 12 pilot evaluator, 11 longer-session evaluator, 5 native Codex boundary and 22 native lifecycle checks. The native Codex checks use a synthetic loopback provider; evaluator self-tests use deterministic fixtures. These establish implementation and reporting behavior, not real-model long-session outcomes. The optional large-store benchmark was not rerun; hosted CI, Linux execution and online dependency advisory scanning remain unverified.

The official [Codex app-server documentation](https://learn.chatgpt.com/docs/app-server) describes usage and compaction events. Current fields were also checked against schemas generated by the installed 0.162.1 executable; newer documentation is not a compatibility guarantee for this pinned release.

**Ready for an extended opt-in pilot; Phase 4 validation is incomplete.** The earlier real-model pilot supports limited normal managed-console use on its tested setup. New optimizations have controlled runtime evidence but still need real-model revalidation. Natural automatic recovery, comparative benefit, full agent/tool resource cost, broader workloads and interruption/recovery under prolonged use remain open. Complete paired real-provider trials, including observed automatic compaction, before claiming long-session or general everyday-production readiness. No other agent integration or enforcement capability was added.

# Real-world Codex pilot

Status on 2026-10-17 (Asia/Calcutta): **login recognized; real-world milestone blocked by this execution sandbox's filesystem and network restrictions**. Tested executable: Codex CLI 0.162.1; Node 24.17.0; Windows; SwapCtx release build with Rust 1.99.0. No storage schema or dependency changes were made.

## Observed result

The original attempt reported `Not logged in` and `authentication_required`. The authenticated follow-up identified an environment mismatch: the tool process runs as `CodexSandboxOffline`; native Codex defaults to that account's home, while `USERPROFILE` identifies the laptop user's profile. The authenticated cache is under the laptop user's Codex home. An unset `CODEX_HOME` therefore selects a different native home. PATH Codex 0.163.0 exhibited the same unauthenticated default; compatibility with that release was not evaluated.

With a process-local `CODEX_HOME` pointing at the laptop user's existing home, pinned Codex 0.162.1 `login status` exits **0**. The login is recognized. However, app-server startup then terminates with filesystem/database access-denied diagnostics. Independently, unauthenticated HTTPS probes to `chatgpt.com` and `api.openai.com` fail locally with **`EACCES`**. This establishes restrictions on this execution process, not a network problem with the user's ordinary terminal, an invalid login or an unavailable model entitlement.

The real-provider control could not initialize the app-server. The coding pilot independently stopped at initialization with `codex_startup_permission_denied` before requesting model turns, creating a SwapCtx store or starting coding. Its disposable project was removed successfully. Logging in again is not a demonstrated remedy for these restrictions.

Reproducible follow-up evidence:

- [Default-home diagnostic](evidence/codex-environment-2026-10-17-default.json): native home differs, login absent, both network probes denied.
- [Authenticated-home diagnostic](evidence/codex-environment-2026-10-17-user-home.json): login recognized, startup permission denied, both network probes denied.
- [Authenticated-home pilot attempt](evidence/codex-pilot-2026-10-17-authenticated-home.json): blocked at startup, no model/coding evaluation started, cleanup completed.

The [original machine-readable pilot result](evidence/codex-pilot-2026-10-17.json) is retained as historical evidence. Both pilot reports record `status: "blocked"`, `modelEvaluationStarted: false`, `codingWorkflowStarted: false`, empty checks/operations and false context-use/adherence flags. Their UTC timestamps fall on October 16; the local verification date is October 17. Credentials, account email, prompts and native configuration contents are absent from the reports. No credentials were copied, replaced or printed, and no existing SwapCtx store was used.

| Requested outcome | Current evidence |
| --- | --- |
| Real-model delivery and requirement adherence | Blocked; no real-model turn completed |
| Ordinary workspace editing | Blocked; no AI-generated code evaluated |
| Authenticated resumption and compaction recovery | Blocked; previous synthetic-provider transport evidence remains separate |
| Preservation during longer development sessions | Unverified; neither a six-turn pilot nor a longer soak completed |
| Everyday latency and provider reliability | Unverified; local timing instrumentation is implemented and regression-tested |
| New pilot evaluator | Twelve offline evaluator tests pass; these are not model evidence |

No synthetic results substitute for any blocked outcome. The managed console remains an opt-in pilot, without production clearance, demonstrated general adherence, enforceable project rules or guaranteed recovery.

## Improvements made

The real-provider pilot now uses the actual Rust managed-console entry point for coding, with an independent disposable project/store. It first runs the existing no-context/unpredictable-identifier control. Six subsequent coding turns extend a small CommonJS text library; every turn is a fresh console invocation resuming the same native thread. A manual compaction separates the first three turns from the last three. A saved release-identifier requirement is superseded after compaction, so later executable checks require new information absent from earlier prompts and implementations.

Independent checks verify normalized text behavior, TypeErrors, preservation of earlier APIs, the current saved identifier, and an unchanged contract file. Unexpected paths, links, missing completion events, missing measurements and changed thread IDs fail the pilot. Passing assistant prose is insufficient. The evaluator also rejects premature zero-status process exits that skip its assertions. This is a bounded ordinary coding exercise; success would not establish hours-long stability, adversarial robustness or natural automatic-compaction recovery.

The adapter emits two additional version-1 events:

- `operation_metrics`: operation, success, last phase, `contextMs`, `sessionMs`, `turnMs` and `totalMs`. Measurements use a local monotonic clock. Null fields mean that phase did not complete. Total includes closing the app-server; it does not isolate provider latency or measure hidden model state. Failures can emit metrics without any context delivery.
- `tool_completed`: observed tool kind, an allowlisted status and an integer exit code when provided. Commands, arguments, errors and tool output are omitted. A completed command with a nonzero exit code remains a failed command. Events cover supported observed item types, not every possible agent action or an enforcement mechanism.

These events make slow operations and tool failures visible while preserving existing events. The evaluator records safe metadata, hashes, stage checks and measurements instead of raw transcripts. A failed later context evaluation retains earlier observed successes while keeping overall `ok: false`. No-auth providers are refused as authenticated evidence; custom authentication modes that the pilot cannot verify are reported as blocked rather than silently credited. Ordinary managed-console support for custom providers is unchanged.

The authenticated follow-up also adds `scripts/diagnose-codex.cjs`: it compares the native home with the requested/user-profile home, checks native login status and initialization, and optionally probes public HTTPS endpoints without authentication headers or model requests. It saves booleans/error identifiers instead of home paths or account details. The diagnostic does not itself read credential contents or copy them; Codex reads its own authentication normally. It does not change global configuration. Initialization can attempt native cache/database writes, so an existing login alone is insufficient.

The adapter now reports `codex_startup_permission_denied` when an app-server disconnect before initialization follows recognized permission-denial markers on stderr. It retains only a bounded transient tail to recognize fragmented markers and never emits raw stderr. This is a diagnostic inference, not proof of which file failed. Markers after successful initialization do not change later disconnects into startup errors. The pilot classifies this prerequisite failure as blocked, without claiming any model turn succeeded.

The offline evaluator tests reject stale identifiers, formatter regressions, missing features, contract mutations, extra files, skipped assertions, absent lifecycle evidence and no-auth providers. Its oracle child receives a minimal environment without inherited API keys and uses Node's filesystem permissions without write access; these restrictions were tested. This is not a complete OS security sandbox or tamper-proof assessment of hostile generated code. The pilot uses Codex's normal configuration/authentication/provider and runtime history; that runtime may initialize configured tools or retain generated probe conversations. Existing project stores and source files are not supplied to the pilot.

## Reproduction

Use an ordinary authenticated Windows PowerShell session with permission for Codex's own home/cache/history writes and outbound provider connections, or an execution profile that grants those capabilities. The current sandbox cannot complete the pilot. Do not place credentials in SwapCtx records or reports. Official OpenAI documentation describes [Codex authentication and credential storage](https://learn.chatgpt.com/docs/auth) and the [account/thread app-server protocol](https://learn.chatgpt.com/docs/app-server).

```powershell
$Codex = 'C:\absolute\path\to\codex.exe' # tested version: 0.162.1
# Only if the native home differs from the home you already authenticated:
# $env:CODEX_HOME = 'C:\absolute\path\to\your-authenticated-codex-home'
& $Codex login status
node scripts/diagnose-codex.cjs $Codex --network
node scripts/check-pilot.cjs $Codex target/release/swapctx.exe .build/NEW-pilot-result.json
```

The diagnostic also accepts `--codex-home ABSOLUTE_DIRECTORY` as a child-process-only override and `--output ABSOLUTE_NEW_FILE`. It does not persist the override or copy credentials. Network probes are optional and diagnostic: direct public-host connectivity can differ from a configured proxy/custom provider, and a response does not establish inference entitlement. The diagnostic's `preflight_passed` status never means a model evaluation passed.

The report destination is optional, its parent must exist, and existing files are refused. Each run owns a fresh temporary project and store and cleans them afterward, including on ordinary failure. Reports retain stage evidence, not source snapshots or raw errors; a failed artifact check may need a deliberately instrumented rerun to investigate. The pilot submits five context-control operations, six coding turns and one manual compaction if every gate passes; native retries or automatic compaction may make additional provider requests. It uses the configured provider and its billing. No substitute model is selected. The pilot currently recognizes native OpenAI API-key/ChatGPT account modes; other provider authentication requires separate validation.

Exit 0 means all pilot gates passed; exit 2 means a recognized authentication or startup-permission prerequisite is blocked; exit 1 means failure. `contextUseObserved` refers to the control's particular successful turns. `workspaceAdherenceObserved` requires all six coding stages. Partial passes remain in `checks`, but never make overall `ok` true. Automatic compaction and long-running-session flags remain false even on a successful bounded run because this evaluator does not establish those outcomes.

The ordinary fail-fast verification script includes offline evaluator checks under `-ContextTests`. `-PilotTests -CodexPath ABSOLUTE_EXE` explicitly adds the real-provider coding pilot. `-ModelTests` remains the smaller context-use evaluation; using both switches runs both requested evaluations. Neither real-provider evaluation runs in CI by default.

## Regression verification

The following passed on Windows after the adapter instrumentation changes:

- Formatting, Clippy with warnings denied, release build, and all 47 native tests in debug and release. The optional performance benchmark remained ignored.
- Ten supplemental SQL checks, nineteen context/adapter checks, eighteen session/metadata/observability checks and twelve pilot-evaluator tests. Three new session tests cover permission diagnostics, fragmented stderr and avoiding misclassification after successful initialization.
- Five prior native Codex boundary checks and seventeen native lifecycle checks. These use a synthetic loopback provider and demonstrate transport only; the lifecycle suite now checks successful measurements and failed-context phase reporting too.
- JavaScript syntax checks and Git whitespace checks.

The authenticated follow-up reran the complete verification command with the updated adapter and all twelve evaluator tests. Hosted CI, Linux execution and online dependency advisory scanning remain unverified, as previously reported. These successful regression suites are not substituted for the blocked real-model gates.

## Recommended next milestone

Run the authenticated pilot in an execution context with the required filesystem/network access before expanding integrations or changing readiness claims. Then investigate any observed coding/compaction failures, run repeated and longer development sessions including naturally triggered automatic compaction, and measure context retrieval versus session startup and model/tool time. Validate compatibility with a newer Codex release separately; an installed newer binary does not establish protocol compatibility. Preserve the existing version restriction until that evidence exists.

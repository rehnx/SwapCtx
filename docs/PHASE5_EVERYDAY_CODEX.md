# Phase 5: everyday Codex integration and live observatory

For subsequent budget-selection fixes, fingerprint v2 compatibility and earlier session-reference validation, see [retrieval and session reliability](RETRIEVAL_SESSION_RELIABILITY.md). The Phase 5 counts and evidence below retain their original scope.

Windows, 2026-10-17. Rust 1.99.0, Node 24.17.0; Codex protocol remains pinned to **0.162.1**. This milestone used **offline deterministic verification only**. No Codex executable, external agent, model/provider, network diagnostic, soak evaluation or soak self-test was invoked. Earlier authenticated pilot evidence remains historical evidence, without replaying expensive evaluations.

## Everyday use

Build from the directory containing Cargo.toml, using cached dependencies:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -DeterministicOnly -ReleaseTests
$SwapCtx = (Resolve-Path .\target\release\swapctx.exe).Path
```

The script discovers the pinned local toolchain and `.build/cargo-home` cache when available. `-DeterministicOnly` enables offline dependency resolution and the deterministic context checks; it rejects `-CodexPath`, `-ModelTests`, `-PilotTests` and `-Benchmark` before executing checks, and skips the soak harness. A machine without the cached toolchain/dependencies must install them separately.

Keep `$SwapCtx` set to that absolute path. From your project's root, register it if it is new:

```powershell
Set-Location 'C:\path\to\your-project'
& $SwapCtx init
& $SwapCtx memory add --kind requirement --key public-api --source 'project-spec' 'Preserve the public API'
```

Existing registered projects and saved adapter settings continue to work. For a first setup, or to explicitly choose a Codex home and enable ordinary workspace editing:

```powershell
$Codex = 'C:\absolute\path\to\codex.exe' # Codex CLI 0.162.1
$CodexHome = Join-Path $env:USERPROFILE '.codex' # existing, authenticated home
& $SwapCtx codex setup --codex $Codex --codex-home $CodexHome --workspace-write --replace
```

Choose the executable/home you normally use. `--replace` preserves a copy of changed settings; omit `--workspace-write` for read-only operation. Setup runs the version check, but makes no model request and does not edit native configuration/hooks or copy credentials. Settings without `codexHome` retain inherited `CODEX_HOME`/native behavior. An explicit home overrides only `CODEX_HOME` in the child environment; native sandbox restrictions remain intact. If a saved session belongs to a different home, `run --new` retains its reference and starts a new thread instead of retargeting it. Changing the setting does not migrate native history or authentication.

Daily commands, from the registered project or any nested directory:

```powershell
& $SwapCtx codex status             # local context/settings/reference checks; no Codex process
& $SwapCtx codex run --observe      # managed console; normal prompts use your Codex provider
# In a second terminal, set $SwapCtx to the same absolute executable and enter the project:
& $SwapCtx codex watch              # local terminal observatory; Ctrl+C exits
& $SwapCtx codex --json watch --once # one machine-readable local observation
```

Use the same absolute global `--data-dir` on every command if you have an alternate store. `codex --node ABSOLUTE_NODE_EXE` overrides Node discovery. The executable embeds both adapter modules; no adjacent integration source files are needed.

The console accepts one prompt per line:

| Command | Behavior |
| --- | --- |
| `/help` | List commands locally. |
| `/status` | Current selection coverage and saved session reference; no agent starts. |
| `/context` | Preview the selected memory, including saved text, locally. |
| `/query WORDS`, `/query` | Set/clear literal retrieval terms for this invocation. Mandatory requirements remain independent of relevance terms. |
| `/remember TEXT` | Explicitly save a note with self-reported source `managed-console`, actor `user`. |
| `/requirement TEXT` | Explicitly save a requirement with that provenance. |
| `/observe on`, `/observe off` | Enable/disable the local observation snapshot. |
| `/new` | Request a new thread on the next prompt/compaction, preserving the old reference. |
| `/compact` | Request native compaction; this can consume model tokens. |
| `/send TEXT` | Send literal prompt text, including a prompt beginning with `/`. |
| `/exit` | Close the console. |

Unknown slash commands fail before starting Codex, avoiding accidental requests for mistyped commands. Console failures remain fail-fast: exit 1, or 130 for a handled interruption. Inspect the error, correct the problem, then run again to resume the saved reference; SwapCtx does not automatically resubmit failed turns. Memory writes are explicit, limited to 4 KiB through the convenience commands, and use the existing transactional Rust implementation and secret guard. No transcript ingestion or automatic model-generated memory capture is added.

Use the full memory CLI for keyed records, review, invalidation, expiry or supersession. Adding a second unkeyed requirement does not supersede an earlier one:

```powershell
& $SwapCtx memory recent
& $SwapCtx memory supersede 1 'The revised public API requirement' # use the actual record ID
& $SwapCtx memory history 1
& $SwapCtx doctor
& $SwapCtx backup 'C:\existing-backup-directory\NEW-swapctx.db'
# After an interrupted process, only if a managed lock remains:
& $SwapCtx codex recover-lock
```

The next managed turn reads fresh context and acknowledges any required update before requesting a turn. An in-flight turn continues with its selected snapshot; concurrent memory edits take effect on the next operation. Startup, resume and compaction architecture remains as documented in [Phase 3](PHASE3_CODEX_LIFECYCLE.md). Selection excludes stale, invalid, conflicting, secret-like or unknown-origin knowledge; incomplete mandatory coverage stops before launching Codex. Byte budget remains 8,192 with at most 100 records, not a bound on total conversation tokens. Saved-reference `contextCurrent` means its fingerprint matches the current selection, including retrieval terms; it does not certify semantic truth or native model state.

## Observatory and diagnostics

Observation is off by default. `run --observe` or `/observe on` publishes a project/store-scoped `*.observatory.json` beside adapter metadata. It is a versioned **latest snapshot**, not an audit log. It retains allowlisted operation state, context hash/size/counts, thread ID, configured model identifier, completed/failed operation counts, observed compactions/tool failures, native numeric token counters and adapter-only timing/CPU/RSS measurements. Prompts, saved record text, assistant messages, provider error messages, command lines, tool output and file contents are not saved in this snapshot.

Ordinary metadata updates coalesce over 250 ms; operation completion/failure and closure flush immediately. Assistant text deltas cause no snapshot writes. Idle snapshots have no heartbeat. `watch` reads locally once per second and reports snapshot age and whether its writer PID exists. PID existence is not proof that Codex is active, and PID reuse is possible. Last-input and cumulative usage counters are native reported counters, **not hidden context occupancy, remaining budget or billing estimates**. The watcher adds no model requests or instructions.

Snapshot reads are bounded to 16 KiB, use strict UTF-8, reject symlinks/non-files/foreign or future formats, and discard unrecognized fields. Publication is atomic. Changed owner/schema metadata disables subsequent writes without replacing the unexpected file. Write failures emit `observatory_write_failed` and disable observation while allowing the managed operation to continue. Remove nothing automatically: inspect and preserve suspect metadata before a manual reset. Initial invalid observation metadata refuses opt-in startup and releases its managed lock. `/observe off` retains a final disabled snapshot; normal exit retains a closed snapshot. These checks are cooperative safeguards, not tamper resistance.

Failed native turns still return the compatible `codex_turn_failed` code. They now also emit `turn_failed` with an allowlisted status/category and, for recognized connection errors, numeric HTTP status. `runtime_error` reports safe categories and native `willRetry`; it does not trigger a SwapCtx retry. Unknown categories become `unknown`; raw messages/details remain withheld. Unsupported hook statuses are similarly reduced to `unknown`. Thus a future console failure can distinguish a reported usage limit from a stream disconnection when Codex supplies that information. A generic RPC/disconnect error can still lack a detailed cause.

Full console `--json` output still includes streamed assistant text and explicit `/context` previews. Redirecting it is **not** equivalent to saving the redacted observatory snapshot. Database, adapter metadata and backups remain plaintext with inherited local permissions. Codex retains its own provider/network/logging behavior; SQLite backup/export does not include adapter metadata or native Codex history.

## Offline Phase 4 evidence investigation

The two supplied local reports were inspected without modification or replay. Their counts and file hashes are preserved in [the offline audit](evidence/phase4-offline-audit-2026-10-17.json). Reproduce that read-only audit while the source reports remain available:

```powershell
node scripts/audit-phase4.cjs .build/soak-20261017-033834.json .build/soak-full-20261017-035118.json
```

| Retained run | Managed checks | Baseline checks | Result |
| --- | --- | --- | --- |
| Six requested turns per arm | 6/6 passed | 6/6 passed | Complete pairs; no automatic compaction observed. |
| Twenty-four requested turns per arm | 21/21 retained turns passed | 22/22 retained turns passed | Interrupted by `codex_turn_failed`; incomplete comparison. |

Neither report records natural automatic compaction or demonstrated recovery after it. The short run has behavior pass difference **0** and reported input-token difference **+1,358** for managed versus baseline; it does not demonstrate a benefit. The longer run has unequal retained workloads, so its cumulative totals cannot establish relative cost or adherence improvements. Cumulative token input across requests is not native context occupancy.

The longer report contains only the generic failure code, without the failed turn's native category/message or failed-operation details. Its root cause cannot be recovered from that artifact. The retained successful checks are not evidence that the unrecorded failed operation passed or that a requirement caused the failure. New console failure diagnostics improve future observation; they cannot reconstruct missing historical evidence, and the legacy soak runner has not been changed or rerun in this milestone.

## Verification and readiness

`verify.ps1 -DeterministicOnly -ReleaseTests` passed format, Clippy with warnings denied, release build, and **47 native tests in both debug and release** (the optional performance benchmark stays ignored). Supplemental checks passed: **10 SQL, 19 context/legacy adapter, 23 session transport, 23 everyday/observatory/evidence, 12 pilot-oracle fixture checks**. After the final dangling-link safeguard, the release executable was rebuilt and the affected transport and everyday suites passed again, including the added live-watcher shutdown test. The deterministic-mode rejection guard was separately verified. [Verification record](evidence/phase5-offline-verification-2026-10-17.json).

New fixtures exercise the real compiled CLI in disposable projects with deliberately nonexistent Codex executables; protocol operations use in-process fake APIs, not native agent processes. They cover local commands/privacy, legacy settings, explicit-home/environment checks, supersession refresh, acknowledged injection, policy preservation, compaction events, scope refusal, safe failure categories, snapshot coalescing/ownership/corruption and watch output. A Node-only watcher fixture verifies polling, changed-snapshot delivery and handled interruption without Codex. Ten thousand text deltas produced no additional observation writes in the controlled fixture.

These checks establish local behavior and the requests this adapter constructs. They do not newly demonstrate native receipt, real-model adherence, editing or compaction behavior. Those retain their earlier evidence and limits. Natural real-model automatic recovery, a measured comparative benefit, broader versions/workloads, total agent/tool resource cost, hosted CI/Linux execution and online dependency advisory scanning remain unverified. No native TUI hook, guaranteed adherence/recovery, hidden model-state visibility or enforceable protection is claimed.

**Ready for opt-in everyday managed-console use on the pinned setup**, supported by the prior authenticated pilot and this milestone's offline regression tests. Broad production readiness remains conditional on the listed limitations. The next milestone should prioritize release packaging and small user-driven fixes informed by optional local observation; there is no need to repeat expensive long-session evaluations as part of this milestone. Any future real-model validation requires a new explicit opt-in. Ordinary `codex` launches still do not activate SwapCtx.

# Phase 8.5: targeted native integration verification

Date: 2026-10-18. Scope: the latest local worktree, Windows x64, deterministic
offline verification only. No provider executable, authenticated session, model,
API inference, soak, package publication or portable distribution rebuild ran.

## Assessment of Phase 8

The original [evidence](evidence/phase8-native-offline-2026-10-18.json) records 22
native tests plus Rust, protocol and distribution checks. Its two retained log
hashes match. Existing tests already cover read-only retrieval, nested-project
scope, all five lifecycle sources, mandatory coverage, stale/conflicted/sensitive
data, updated requirements, malformed inputs, timeout bounds, fallback, provider
errors, interrupts and temporary-directory containment. These are offline
adapter results, not evidence from Codex or Claude.

The current local `src/main.rs` contained conflict markers between `launch` and
`native`, so historical evidence alone could not establish a buildable current
tree. No repository checkout or Git operation was used to replace local work.

| Confirmed issue | Fix and evidence |
| --- | --- |
| Unresolved Rust dispatch conflict prevented compilation | Repaired the working file, retained both commands and the existing managed console, reused the selector with the opt-in context prompt. Bare nonterminal invocation still refuses launch; both Enter and q cancel. Current build, Clippy and legacy launcher tests pass. |
| Duplicate JSON keys could hide sensitive or conflicting text from `JSON.parse` validation while original bytes were forwarded | Reject repeated decoded member names at any object depth. Reproduced before the fix; literal and escaped-key fixtures are now rejected. |
| Valid `Requirement` records failed mandatory coverage because the adapter counted only lowercase `requirement` | Match existing Rust/SQLite case-insensitive semantics without weakening coverage equality. A real disposable-store regression reproduced the failure. |
| A JSON packet exactly at the byte limit failed because stdout's newline was counted as payload | Allow one LF or CRLF outside the unchanged 7,168-byte JSON budget. Full-budget fixtures now pass; excess transport output is rejected. |
| A success receipt preceded stdout, and an asynchronous broken pipe could escape error handling | Record `context_prepared`, await writes and handle stream errors. An EPIPE fixture now records the preparation and output failure, without claiming emission or provider receipt. |
| Claude `-c` was rejected by the Codex config-override guard | Apply that short-option restriction only to Codex. Claude's original continue flag reaches the fake launcher unchanged; workspace relocation and Codex overrides remain rejected. |

All five adapter regressions were reproduced before their fixes. Existing tests
were strengthened with representative native metadata, absent/null transcript
paths, existing project configuration/instruction sentinels and the configured
Codex Windows command. No new retrieval feature, schema, dependency, native UI
or provider policy override was added.

## Native contract evidence and limits

Both current hook references document JSON stdin containing `hook_event_name`,
`cwd` and a SessionStart `source`, with session/transcript metadata. The adapter
uses only event/source/scope, ignoring optional metadata and never reading the
transcript. Synthetic fixtures exercise its real Node entry point.

Both document `startup`, `resume`, `clear`, `compact`, and `fork`, and JSON stdout
at `hookSpecificOutput.additionalContext` with `hookEventName: "SessionStart"`.
Codex describes root-compaction delivery before continuation. Its hooks require
trust of the current definition; project trust and managed restrictions also
apply. The adapter neither grants trust nor enables disabled hooks. Inline
session configuration and Windows command overrides are documented. Exact
provider merge, parsing and trust behavior remain untested here.
[Codex hooks reference](https://learn.chatgpt.com/docs/hooks).

Claude documents executable-plus-args command hooks, including real `.exe`
requirements on Windows. Its SessionStart fields can omit the model, and fork
used `resume` before 2.1.214. The temporary plugin uses the documented context
output only, with no permissions decisions or environment-file writes.
[Claude hook reference](https://code.claude.com/docs/en/hooks).

`--plugin-dir` is session-scoped; `-c` is Claude's continue shorthand.
[Claude CLI reference](https://code.claude.com/docs/en/cli-reference).
Administrator controls may disable local plugin loading or restrict hooks.
Interactive trust/policy behavior for the temporary plugin is still a live
provider gate, not something synthetic tests establish.
[Claude plugin security](https://code.claude.com/docs/en/plugins/security).

| Provider | Versions admitted by the existing probe | Supported-version evidence |
| --- | --- | --- |
| Codex | Recognized semantic version plus help containing `--config` and `--dangerously-bypass-hook-trust`; that bypass flag is never supplied | No exact release or minimum certified. Help fixtures prove the gate, not that any real release implements the full contract. The prior managed-console pin 0.162.1 is not native-hook evidence. |
| Claude | Major 2, at least 2.1.296, plus `--plugin-dir` in help | An implementation admission floor only. The reviewed docs establish exec form but do not establish 2.1.296 as its minimum. The admitted range remains unverified. |

No live-provider support matrix is claimed. Unknown/failed probe results retain
ordinary native launching with a warning. A candidate may still reject or skip
hooks; provider exit status is retained without automatic relaunch.

Both references specify hook timeouts in seconds. Generated handlers request 10
seconds; the adapter independently bounds stdin at 32 KiB/3 seconds and retrieval
at 5 seconds. Validation failures emit a safe advisory and no context, normally
exiting zero. A broken stdout exits unsuccessfully and may have written partial
output. Provider-owned timeout cancellation/error presentation is unverified.
Withholding context does not prevent the provider from continuing.

Diagnostics now distinguish **context prepared / hook code executed** from
**provider receipt unknown / model adherence unknown**. Missing receipts report
no recorded callback. Same-user receipt tampering, hard termination, natural
compaction scheduling and other providers' hooks are not certified by these tests.

## Local verification

| Command / check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --offline --all-targets -- -D warnings` | Passed |
| `cargo build --locked --offline` | Passed; development binary only |
| `cargo test --locked --offline --all-targets` | 54 passed, one existing performance test ignored |
| `node scripts/check-native.cjs target/debug/swapctx.exe` | 27 passed (22 existing cases, some strengthened; five defect regressions) |
| `node scripts/check-context.cjs target/debug/swapctx.exe` | 23 passed; protocol/dispatch regression coverage |
| Release source inventory / embedding inspection | Native adapter and both Rust launchers included; no packaging defect found |
| Historical Phase 8 log hashes; `git diff --check` | Passed |

Rust checks used the installed 1.99.0 toolchain and cached Cargo dependencies;
Node was 24.17.0. For this environment, prepend the installed toolchain's `bin`
directory to PATH and set `CARGO_HOME` to `.build/cargo-home` before the commands.
The native suite intentionally prints `hook_input_timeout` for its timeout
fixture. Its process exit is zero. No expensive evaluations or package suites
were repeated. See [current evidence](evidence/phase85-native-offline-2026-10-18.json).

## Files and readiness

| File | Purpose |
| --- | --- |
| `integrations/native/native.cjs` | Packet ambiguity, kind/byte accounting, stdout handling, truthful receipts, provider-specific short flag |
| `scripts/check-native.cjs` | Five reproduced regressions; representative metadata, configuration sentinels and Windows callback assertions |
| `src/main.rs` | Working-file conflict repair retaining both command families and accurate capability text |
| `src/native.rs`, `src/provider.rs` | Reconcile the two selectors while preserving explicit opt-in, cancellation and legacy launching |
| `docs/PHASE8_NATIVE_PROVIDERS.md` | Current diagnostics, version caveats and link to this follow-up |
| This report and `docs/evidence/phase85-native-offline-2026-10-18.json` | Assessment, reproducible checks and separate current evidence |

**Ready as an offline-tested Phase 9 foundation, not a certified live native
integration or public release.** Remaining gates are real-provider configuration,
trust, lifecycle receipt and model-behavior verification (only in a separately
authorized future task), plus clean-machine/hosted acceptance when appropriate.
The complete positive path through Rust, provider probing and a real provider's
configuration loader remains unverified; fake callback tests do not substitute
for it. Packaging was inspected, not rebuilt or recertified.

The Git index still records the pre-existing unmerged `src/main.rs`; only the
working file was repaired. All changes remain local for review. No commit,
rebase, reset, index resolution, push, merge, publication or release was performed.

# Codex hook delivery investigation

Date: 2026-10-20. Local HEAD: `b84b3cd`, with the prior shell fix and existing
uncommitted CLI/documentation changes preserved. This investigation used local
fake executables and synthetic events only. No installed provider was executed,
including help/version probes. No model request was made.

## Conclusion and attribution

**The cause of the second real run remains unknown.** The user reports a passing
local health check, successful native launch, valid isolated memory and no recorded
callback. No `/hooks` state or hook stderr was retained. These observations cannot
distinguish disabled/untrusted hooks, loading failure, command startup failure,
configuration failure, interruption or receipt logging failure.

The prior [Windows shell failure report](CODEX_WINDOWS_HOOK_FAILURE.md) remains
historical evidence. Its offline shell fix is preserved. This investigation found
no evidence that the previous quoting failure recurred in the second run.

Two confirmed diagnostic gaps were addressed:

1. Receipts started after config validation. Successful Node entry followed by
   invalid configuration was indistinguishable from a skipped hook at native exit.
2. Successful hook execution with unavailable receipt storage could leave no receipt
   despite valid context reaching stdout. The old absence warning could not identify
   that case after the temporary files were cleaned.

These are reproduced local failure modes, **not attribution of the live incident**.
No provider acceptance, receipt, model use or adherence is certified.

## Contract review: Codex 0.163.0

The version-pinned [hook configuration types](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/config/src/hook_config.rs)
declare `SessionStart`, command handlers, `command_windows` as an alias of
`commandWindows`, seconds-based `timeout`, and `additionalContextLimit`.
SwapCtx retains its invocation-only `-c hooks.SessionStart=[...]`, anchored matcher
for startup/resume/clear/compact/fork, ten-second timeout and complete bounded output.
No registration spelling/shape defect was found by this static review.

The pinned [discovery implementation](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/hooks/src/engine/discovery.rs)
reads inline hooks from config layers, classifies session flags as non-managed,
selects the Windows command and gates execution on enabled/trusted state and managed
policy. Its normalized trust hash includes the command definition. **SwapCtx's
random temporary paths change that definition each launch.** Prior approval cannot
establish trust for the next launch. This is a verified design limitation and a
plausible cause of a skipped callback; it was not observed in the second live run.
This change adds explicit guidance and does not bypass trust or install stable hooks.

The pinned [SessionStart implementation](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/hooks/src/events/session_start.rs)
uses the five source names as matcher input, serializes event JSON and processes
successful hook output as context. The [command runner](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/hooks/src/engine/command_runner.rs)
pipes JSON stdin/stdout/stderr, replays filtered environment, and dispatches `/C`
with a raw quoted command or other shells with a normal argument. The existing
Rust shell fixture exercises both Windows branches and the historical cmd branch.
The source contract is not execution of the installed binary, its policy or parser.

The current [official hooks guide](https://learn.chatgpt.com/docs/hooks) also
describes exact-definition trust, `/hooks` review, disabled hooks and managed-only
policy. These mutable docs supplement, rather than replace, the pinned source.

## New observations and their limits

`configure()` creates `startup.bin` exclusively in the owned temporary hook
directory. It contains exactly eight bytes: the format marker `SCX1` and four
boolean flags. Windows encoded bootstrap marks entry before trying Node; Node
marks entry before config loading, then successful config validation. Failed
receipt writes mark the last flag. Repeated callbacks set existing flags and
cannot grow the file. No input, context, session ID, paths, prompt or credential
is stored in it.

Node only updates an existing regular, correctly sized/formatted file and checks
the opened file identity. Missing, malformed or redirected diagnostic targets do
not gate validated context or create replacement files. The Windows bootstrap
uses .NET APIs and rejects a final reparse point. Permission/interruption/race
failures can still lose observations. This is best-effort metadata, not durable
audit evidence, protection against same-user tampering or provider confirmation.
No ACL changes or permissions bypass were added.

After the provider exits, SwapCtx reads the flags **before cleanup**, prints a
fixed-field summary without paths, and removes the metadata with the existing
owned-directory cleanup. The flags mean “observed at least once in this launch,”
not counts, chronology or proof that every callback reached a stage. Claude's
direct exec path correctly leaves Windows bootstrap false.

| Observations | Next evidence or interpretation |
| --- | --- |
| All flags false | Loading/trust/policy, shell startup or inaccessible diagnostics remain possible; inspect `/hooks` and stderr |
| Windows bootstrap true, Node entry false | Bootstrap was observed; inspect missing Node/adapter, runtime or startup errors |
| Node entry true, config validated false | Adapter entered; inspect `hook_config_invalid` or interrupted configuration loading |
| Config validated true, receipt write failed true, no receipt | Hook reached config validation but logging failed; hook output may still have succeeded |
| Config validated true, no receipt, no failure flag | Metadata may have been removed or a process interrupted; do not infer skipped execution |
| Receipt `callback_requested` only | Adapter/config reached; input may be waiting, timed out or interrupted |
| `context_withheld` | Inspect the bounded safe error code; context validation/input/retrieval failed |
| `context_prepared`, then `output_failed` | Local output failed and may be partial |
| `output_written` | Local stdout write completed; provider acceptance and model behavior remain unknown |
| Startup metadata unavailable/invalid | Early observations are incomplete; absence is inconclusive |

Existing safe bootstrap codes (`hook_adapter_unavailable`,
`hook_node_start_failed`), configuration code (`hook_config_invalid`), receipt
warnings, validation codes and output errors remain. OS/provider stderr can
include paths or output, so inspect locally and redact before sharing.
`--check` still proves local discovery/runtime/store/context/config generation
only. It does not run a provider, decide trust or prove shell dispatch.

## Offline evidence

| Verification | Result |
| --- | --- |
| Node syntax checks | Passed |
| Focused Windows shell/startup/missing-callback cases | 3 passed before complete suite |
| `cargo fmt --check` | Passed after removing one pre-existing leading blank line |
| `cargo clippy --locked --offline --all-targets -- -D warnings` | Passed |
| `cargo build --locked --offline` | Passed; debug executable embeds the updated adapter |
| `cargo test --locked --offline --all-targets` | 55 passed; existing performance test ignored |
| `node scripts/check-native.cjs target/debug/swapctx.exe` | 44 passed |
| `node scripts/check-context.cjs target/debug/swapctx.exe` | 23 passed |
| `node scripts/check-docs.cjs target/debug/swapctx.exe` | Passed: 12 files, 59 local links, command and PowerShell syntax checks |

The native suite exercises generated TOML through actual Windows cmd/PowerShell
dispatch, spaces/Unicode/punctuation, earliest stage flags, invalid config,
missing Node/adapter, invalid input, logging failure, broken stdout and existing
lifecycle/privacy cases. The Rust CLI-to-embedded-adapter-to-fake-provider chain
now reproduces invalid config and blocked receipt logging for both providers.
Skipped fake hooks and two fresh configurations demonstrate absence and changing
definitions; they do not implement or certify Codex's actual trust UI/hash store.
Metadata corruption/missing file/junction fixtures verify bounds and no overwrite.
The receipt-lock fixture deliberately confirms that context can be written even
when the parent reports no receipt. No assertions were weakened.

The first Rust test build collided with the native suite holding `swapctx.exe`
open (Windows access denied). It passed when rerun after the native suite exited.
The Node log's `hook_input_timeout` diagnostic/PowerShell NativeCommandError is
expected from the existing negative timeout fixture; all 44 tests and the process
exited successfully. No packaging, release build, benchmark or soak suite ran.

Changes: `integrations/native/native.cjs`, `scripts/check-native.cjs`,
`tests/fixtures/native-provider.cjs`, and this report. `src/display.rs` changed
only through formatting (one leading blank line); Gemini's UI code and the other
saved Rust/managed-console files were checked against their starting hashes.
The reality-check plan and existing documentation were preserved. Antigravity's
branch/worktree was not accessed or changed. No Git mutation was performed.

## Smallest next authorized provider check: no model prompt

**Procedure only. It has not been executed and requires separate authorization.**
Reuse the disposable project and isolated data directory, never the real repository
or default memory. Use the newly built executable by explicit absolute path; record
its SHA-256 with `Get-FileHash` and its build timestamp. An older globally installed
executable does not contain these observations. Do not reuse old temporary hooks.

1. Run local health with exactly the same `--data-dir` and project directory.
   Record the result without copying memory text. Check the provider UI version
   matches 0.163.0; do not change authentication or global settings.
2. Launch that executable with `--data-dir ISOLATED_DATA native codex --context`
   in the disposable project. Keep stdin/stdout/stderr attached to the original
   terminal; do not pipe or start a transcript recorder. Do not type a prompt.
3. Open `/hooks` immediately. Record installed count, active count, source,
   enabled state and exact trust/review status for **this launch's** SwapCtx
   definition. Inspect normal trust UI; never supply a bypass. If policy prevents
   approval or no definition is installed, stop here and record that result.
4. Capture any hook failure detail: exit status, parser/runtime code, timeout,
   output handling and stderr. `/warnings` can expose retained details; inspect
   locally, copying only relevant redacted hook evidence. Do not export history,
   raw context, complete configuration, session transcripts or credential files.
5. If startup was skipped pending trust and normal review is approved, use a
   single **bare `/clear`** in this disposable session if offered by the installed
   UI, with no inline prompt. The pinned TUI [slash command source](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/tui/src/slash_command.rs)
   defines it as clearing the terminal and starting a new chat. This is an attempt
   to obtain a clear lifecycle callback, not proof startup was replayed. Do not use
   `/compact`, `/init`, a goal or a coding prompt: they can invoke a model or write
   instructions. If the UI requires a model turn, stop rather than sending one.
6. Exit normally. Capture the new local stage summary and receipt counts/codes
   from SwapCtx. Do not terminate early before the summary unless necessary.
   Treat inability to capture provider stderr as missing evidence, not a pass.

Objective stopping points: no installed hook means investigate loading/config;
installed but inactive/untrusted means trust/policy; provider reports spawn/parser
failure with no Node observation means dispatch/runtime; Node without validated
config means config loading; validated config with failed logging means receipt
storage; withheld context means its specific input/scope/retrieval/validation code.
All-false flags alone still cannot identify a cause. A confirmed provider callback
with successful local output narrows the remaining question to provider acceptance.
No model inference is needed to investigate these layers. Actual model use and
adherence remain a later, separately authorized one-prompt test only after the
hook chain is evidenced. Do not escalate to expensive lifecycle evaluations.

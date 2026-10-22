# Windows Codex hook launch failure: offline reproduction and fix

Review date: 2026-10-20. Baseline: `b84b3cd`, plus pre-existing local UI and
publication documentation edits. No Codex/Claude executable or version/help
probe was run. The user's reported Codex 0.163.0 session is external evidence:
native UI opened; one SessionStart handler was installed/active; hook exit 1;
one response was `NOT_RECEIVED`; no SwapCtx callback receipt existed.

## Root cause established offline

SwapCtx generated this outer command shape:

```text
"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -EncodedCommand BASE64
```

The encoded payload correctly quoted Node, adapter and configuration paths, but
the **outer command** was not valid under every provider-selected Windows shell.
A quoted executable path followed by flags is not an invocation in PowerShell:
it is a string expression followed by unexpected tokens. PowerShell rejected
`-NoProfile` / `-NonInteractive`, exited 1 and never started the adapter.

The reported [upstream issue #32402](https://github.com/openai/codex/issues/32402)
describes a separate historical failure: ordinary Rust argv escaping changes
embedded quotes before `cmd /C` sees them. That path was also reproduced, but
the issue is closed and cannot simply be attributed to 0.163.0.

The pinned [0.163.0 command runner source](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/hooks/src/engine/command_runner.rs)
uses a quoted `raw_arg` for the default shell or a configured `/c` shell, and
an ordinary argument for other configured shells. Its
[hook registry](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/hooks/src/registry.rs)
accepts the shell program and arguments. The pinned
[shell argument construction](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/core/src/shell.rs)
includes PowerShell `-Command`. These are source-contract observations, not
execution of Codex or proof of which shell the user's process selected.

Before changing adapter code, the new regression ran the actual generated TOML
command through a compiled Rust shell dispatcher and Windows interpreters:

| Offline dispatch | Original result | Earliest receipt |
| --- | --- | --- |
| Current `cmd /C` with outer quotes and `raw_arg` | Success | Recorded |
| Historical `cmd /C` with ordinary `.arg` | Exit 1, escaped executable not recognized | Absent |
| PowerShell `-Command` with ordinary `.arg` | Exit 1, `UnexpectedToken` parser error | Absent |

The prior positive test extracted the executable and encoded argument and called
PowerShell directly. That bypassed both outer-shell parsing and Rust argv
escaping. The old full-chain synthetic provider had the same blind spot.

**Attribution boundary:** the offline PowerShell failure reproduces the reported
exit/no-receipt symptoms and proves a real SwapCtx defect. The supplied live
observations contain neither the chosen shell nor its stderr, so they do not
uniquely establish the original incident's exact branch. Missing receipts alone
also cannot distinguish skipped trust, startup/config failure or unavailable
receipt storage. Do not claim #32402 persists in 0.163.0 or that live receipt is
now fixed without a new authorized test.

## Fix and preserved security boundaries

`hookCommands()` now emits the absolute system PowerShell path as an unquoted
token only after a strict allowlist proves that it is a drive-absolute path
without spaces, quotes, expansion syntax or shell metacharacters. The existing
existence check and system-directory selection remain. No PATH lookup, short-path
trick, cmd-variable expansion, call to a provider or user shell setting is added.
The fixed command works with the two current dispatch branches and the older
cmd escaping path. Node, adapter and config paths can still contain spaces,
apostrophes, Unicode and shell metacharacters: they remain single-quoted inside
the UTF-16LE encoded payload, with apostrophes escaped.

An unusual system-interpreter path outside the allowlist fails closed with
`windows_hook_interpreter_path_unsupported`. In particular, a Windows system
directory containing spaces is not silently executed unquoted. This restriction
applies to the interpreter path, not project, Node or adapter paths. Custom
outer shells/Git Bash and PowerShell 7 were not exercised by this regression.

The encoded bootstrap now reports a missing adapter file or a failed Node start
using fixed safe codes. It uses a .NET file check rather than PowerShell module
autoload. An intermediate attempt using `Test-Path` under stop-on-error hit a
sandbox/short-path current-directory access error; the .NET check resolved it
without changing access permissions or relaxing error handling.

`hook()` maps failures before a trusted config is loaded to `hook_config_invalid`.
It cannot write a receipt before validating the receipt destination. The missing
callback warning now explains the possible pre-callback failures and directs
reviewers to provider hook stderr; health is explicitly not a provider-shell test.

| Observation | Interpretation |
| --- | --- |
| Outer-shell parse/not-recognized error, no receipt | Command launch failure before Node; provider stderr is needed |
| `hook_node_start_failed` | Encoded bootstrap reached the native invocation but could not start it |
| `hook_adapter_unavailable` | Bootstrap found no adapter file; Node adapter did not run |
| `hook_config_invalid` | Node hook entry ran but its config could not be validated; no trusted receipt destination |
| `continuity metadata unavailable` and incomplete marker | Receipt logging unavailable; valid context is still eligible |
| `callback_requested` then `context_withheld` with `hook_input_invalid` | Config/receipt usable, event parsing failed safely |
| `context_prepared` then `output_failed` | Validation succeeded but output failed; partial receipt is possible |
| `output_written` | Local write completed, not provider acknowledgement or model adherence |

Safe codes do not contain paths, memory text, input events or credentials. No new
persistent logs, transcript capture, trust bypass, schema, provider configuration
change or context-validation relaxation was introduced. Provider/OS-generated
stderr may itself include paths; inspect it locally and redact before sharing.

## Regression coverage

`tests/fixtures/windows-hook-runner.rs` models the Rust Windows argument boundary
using only the standard library. The Node test extracts the generated TOML
`command_windows` string; it does not split it into an executable/argument list.
The fixture uses cmd `/D` to disable machine/user AutoRun and PowerShell
`-NoProfile -NonInteractive` for deterministic isolation. It does not implement
Codex's full TOML loader, policy, environment filtering, job objects or UI.

The focused regression covers all three dispatch paths, poisoned cwd/PATH
helpers, script/config paths with spaces and punctuation, Unicode context,
JSON stdin, captured status/stdout/stderr, the earliest callback stage and full
validated context output under the existing byte limits. Re-quoting the fixed
outer token reproduces both earlier failures before any receipt is created.
Negative cases distinguish missing Node, missing adapter, malformed config,
invalid stdin and a blocked receipt lock. Existing broken-output tests preserve
the distinction between preparation and failed emission. The system-interpreter
path guard is tested with a junction whose name contains spaces/metacharacters.

The synthetic Codex provider now executes the generated command through actual
cmd dispatch during the complete Rust CLI → embedded adapter → fake provider →
hook chain. The focused Rust dispatcher additionally covers outer PowerShell.
No installed provider is part of either path.

## Verification and local changes

Commands use the installed pinned Rust toolchain, cached Cargo dependencies and
Node. The regression was first observed failing against the original adapter;
after the fix the focused shell, direct invocation and broken-output cases passed.

```powershell
node --test-name-pattern="generated Codex TOML hook|Windows encoded command|broken hook stdout" scripts/check-native.cjs target/debug/swapctx.exe
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File scripts/verify.ps1 -DeterministicOnly
```

The first full-gate attempt stopped at formatting differences in the pre-existing
Rust UI edits. Those four files were copied under ignored `.build/` before
`cargo fmt`; no UI behavior was intentionally changed. The gate was restarted.
That run passed the new shell regression but failed one existing negative
full-chain case with Windows process status `4294967295` after five failure
observations. A targeted rerun passed all scenarios in that case. The originating
process termination was not established; assertions now identify the provider,
scenario and spawn error code on recurrence. This result is retained rather than
attributed to the shell fix or omitted. A subsequent full-gate result follows.

Final deterministic gate: **exit 0** on 2026-10-20.

| Check | Result |
| --- | --- |
| Rust formatting and Clippy (`--all-targets`, warnings denied) | Passed |
| Rust tests | 55 passed; existing performance test ignored |
| Locked, offline release build | Passed |
| Supplemental SQLite / context checks | 10 / 23 passed |
| Session / everyday checks | 23 / 28 passed |
| Native integration checks | 41 passed, including the earlier failing negative case |
| Package / Windows release checks | 19 / 4 passed |
| Offline pilot-oracle fixtures (no provider) | 12 passed |
| Third-party inventory | 49 locked Windows packages verified |
| Documentation gate | 12 files, 59 local links, 28 fenced blocks checked |

The supplemental suites total 160 passing checks. Full-gate output is retained
locally in `.build/codex-hook-fix-verify-final.stdout.log` and
`.build/codex-hook-fix-verify-final.stderr.log`; the preceding failed gate remains
in `.build/codex-hook-fix-verify.stdout.log` and its stderr companion. These ignored
logs contain development paths and should be reviewed/redacted before sharing.
The expected `hook_input_timeout` diagnostic comes from a negative fixture.
The release executable at `target/release/swapctx.exe` has SHA-256
`BCDC738D7A30A7D5E10C2ADDED2182002771B492A7B970512AF0CDBC94A22487`.
No distribution was published.

Scoped whitespace checks passed for the hook implementation/test changes.
Whole-tree `git diff --check` reports trailing spaces in the user's pre-existing
ASCII banner string in `src/display.rs`; those string contents were preserved.
The four formatted Rust files matched their saved copies with whitespace removed.
The reality-check plan's SHA-256 remained
`585D1F9AD02D3049BF5A81FCE9D33F1E2C2FF0BB8C5F868AE514934CE52C2910`.

Changed for this fix:

- `integrations/native/native.cjs`: outer command compatibility and safe failure diagnostics.
- `scripts/check-native.cjs`: real shell boundary regression, safe-path and diagnostic assertions.
- `tests/fixtures/windows-hook-runner.rs`: new offline Rust dispatcher.
- `tests/fixtures/native-provider.cjs`: actual cmd boundary in the synthetic Codex chain.
- This report.
- `src/display.rs`, `src/main.rs`, `src/native.rs`, `src/provider.rs`: formatter-only changes on top of the user's existing edits.

The local reality-check plan and existing publication documentation were retained.
No commit, push, tag, provider launch or model request was made.

## Repeating the isolated trial

After successful offline verification, the existing isolated test is suitable
for a separately authorized repeat using the **newly built** SwapCtx binary.
Do not reuse an older installed binary or the previous process's temporary hook.
Confirm the binary hash/path, rerun local health with the same isolated data-dir,
launch a fresh native session and review the newly generated hook under normal
provider trust. Do not bypass trust or force compaction. Keep the original
one-prompt budget; submit it only after observing a real callback and local output.
If startup is skipped pending trust, a later clear is evidence for clear only.

Record provider-selected shell/error details, callback stages, output handling
and the marker response separately. An absent callback or another exit error
should stop the model step. Actual Codex 0.163.0 configuration/trust execution,
provider acceptance, model use, recovery events and behavior on other Windows
installations remain unverified by this offline fix.

# Phase 8: native provider UIs

SwapCtx can launch the original Codex or Claude Code terminal interface. The
managed `swapctx codex ...` console remains available with its existing behavior.
The native route is separate and does not implement a replacement chat console.

Phase 8.5's targeted audit and fixes are recorded in
[Native verification](PHASE8_5_NATIVE_VERIFICATION.md). The Phase 8 evidence below
is historical; it does not certify the later local source or any provider version.

## Everyday commands

Install the selected provider separately using its own installation instructions.
Its authentication, settings, permissions, terminal UI and history remain under
that provider's control. SwapCtx does not install or log into a provider.

```powershell
# Interactive selection; memory exposure defaults to No.
swapctx

# Native UIs, without exposing SwapCtx memory. No Node dependency in SwapCtx.
swapctx native codex
swapctx native claude

# Explicitly opt into selected project memory for this invocation.
Set-Location 'C:\Projects\YourProject'
swapctx init  # Once per project; omit for an already registered project.
swapctx memory add --kind requirement --key compatibility 'Preserve public interfaces'
swapctx native claude --context
swapctx native codex --context

# Forward original UI arguments after --, including session resumption.
swapctx native claude --context -- --continue
# The provider's equivalent short form also works.
swapctx native claude --context -- -c
swapctx native codex --context -- resume

# Overrides when native executables are not on PATH. Use actual installed paths.
swapctx native claude --context `
  --executable 'C:\Tools\Claude\claude.exe' `
  --node 'C:\Program Files\nodejs\node.exe'
```

Node.js 24+ is required for the optional context adapter. Ordinary native launch
does not need Node or an initialized database. On Windows, executable overrides
must be absolute existing `.exe` files. Shell shims (`.cmd`, `.bat`, `.ps1`) are
not supported: select the underlying native executable. Default discovery checks
absolute PATH directories and excludes the current project directory, avoiding
Windows' implicit current-directory executable search. SwapCtx does not modify
PATH. Bare invocation requires an interactive terminal; scripts should use an
explicit subcommand.

## Documented interfaces and compatibility

Official documentation reviewed on 2026-10-18:

| Provider | Integration used | Runtime compatibility gate | Unverified boundary |
| --- | --- | --- | --- |
| Codex CLI | Invocation-only `-c hooks.SessionStart=[...]`, including Windows command override; no trust bypass | Recognizable `codex-cli` semantic version and help advertising both `--config` and the modern hook-trust flag | No minimum compatible release was established by the reviewed documentation; advertising flags is only a prerequisite, not a runtime delivery test |
| Claude Code | Temporary local plugin loaded with `--plugin-dir`, containing exec-form command-only `SessionStart` hooks | Claude Code 2.1.296 or later within major 2, and help advertising `--plugin-dir` | This is an implementation admission floor, not an established upstream minimum or a verified compatible range |

**No exact native-provider release is certified.** The documentation establishes
the interfaces; version/help probes only admit candidate versions. In particular,
the reviewed documentation does not tie exec-form hooks to Claude 2.1.296.

The gates use bounded local `--version` and `--help` processes **only when the
user launches with `--context`**. Offline tests substitute fixtures/fake
executables and never execute either provider. An unsupported, unknown, failed or
timed-out probe produces a warning and launches the original UI without context.
It does not change provider settings, attempt authentication or send a prompt.

Codex's [official hooks reference](https://learn.chatgpt.com/docs/hooks) describes
session configuration, hook trust and context output. Its
[CLI reference](https://developers.openai.com/codex/cli/reference) documents
configuration overrides. This implementation uses session configuration rather
than assuming that desktop plugin installation is available in every CLI version.
The historical managed-console pin, Codex 0.162.1, is **not** certified for this
native adapter. A provider may reject an override even after advertising the
required flags; its exit status is preserved and there is no automatic relaunch.

Claude's [CLI reference](https://code.claude.com/docs/en/cli-reference) documents
session-only plugin loading. The
[plugin authoring guide](https://code.claude.com/docs/en/plugins/create) describes
the plugin layout, and its [hooks reference](https://code.claude.com/docs/en/hooks)
defines the event and output contract. Claude hooks use an executable plus an
argument vector, avoiding dependency on Git Bash or PowerShell quoting. Managed provider policy may disable local
hooks or plugins. SwapCtx does not override that policy.

No provider integration has been exercised against a live provider or model in
Phase 8. These are documentation-backed adapters with deterministic local contract
tests, not verified native-model integrations.

## Lifecycle behavior and trust

The configured event is `SessionStart`, matching `startup`, `resume`, `clear`,
`compact` and `fork`. Every callback retrieves a new snapshot. It does not replay
launch-time memory after compaction. This is the intended documented route for
manual and automatic compaction; **actual provider callback execution, model
receipt and post-compaction adherence remain unverified**.

Codex requires review/trust of non-managed hooks. SwapCtx never uses the trust
bypass flag or writes trust records. Review the generated SwapCtx handler in
Codex's `/hooks`. If the initial hook was skipped pending approval, a subsequent
`/compact` requests another SessionStart event. Temporary command paths change
between launches, so approval may be required again. Until a callback succeeds,
memory availability must not be assumed.

There is no prompt interception or per-turn memory injection. Changes saved
during a session are retrieved at its next matching lifecycle event; a supported
provider's `/compact`, `/clear` or session resumption can request a refresh.
SwapCtx does not trigger these operations automatically. Selection uses existing
project requirements plus recent valid records; SessionStart provides no task
query, so this route does not claim task-specific semantic ranking.

The provider inherits the terminal streams and normal environment. Context
integration refuses workspace-relocation and Codex configuration override flags
that could invalidate its scope/configuration. Use ordinary native launch for
those workflows, or change directories before launching. Other user arguments
are forwarded, including additional Claude plugin directories and settings.
Unrecognized provider behavior may still prevent callbacks; inspect the warning
and provider diagnostics rather than assuming delivery.

## Read-only delivery and privacy

`integrations/native/native.cjs` is shared by both providers and embedded in the
Rust binary. It invokes the existing `swapctx context --protocol-version 1`
command with a 7,168-byte JSON budget and at most 100 records. The final context
text is limited to 8,192 UTF-8 bytes, including its disclaimer. Byte counts are
not token estimates.

Validation covers protocol/schema versions, snapshot age, exact canonical
registered-project identity, event scope, strict retrieval policy, mandatory
requirement/rule counts, scan completeness, record freshness/expiry/conflicts,
self-reported origins, sensitive-value heuristics, UTF-8 and byte accounting.
Retrieval at the event directory rejects a separately registered nested project
instead of handing its context to the parent project's integration. Sensitive
strings in provenance or additive fields are also rejected. Duplicate JSON keys
(including escaped spellings) are rejected before forwarding the original packet.
Requirement counting follows the CLI's case-insensitive kind semantics. The JSON
budget excludes the CLI's one trailing LF; a CRLF transport ending is also allowed.
The existing context
command remains responsible for lifecycle/history validation in its read-only
SQLite snapshot; no schema migration or database write is performed here.

Sensitive-data detection remains heuristic. Opting in exposes selected plaintext
memory to the provider, whose configured service may receive it. SwapCtx adds no
network client, telemetry, background service, automatic memory capture or
transcript collection. It does not expand model context or enforce boundary
rules. Provenance is identifiable and self-reported, not authenticated.

Only adapter code, configuration paths, a temporary fingerprint key and bounded outcome metadata are written
to disposable temporary directories. Records, prompts, provider output and
transcripts are not retained by SwapCtx's native adapter. It never writes
`AGENTS.md`, `CLAUDE.md`, global/project provider settings or existing user files.
Normal exits, launch errors and handled interrupts clean temporary plugin state.
Forced process termination or power loss can leave temporary adapter directories;
there is no claim of crash-proof cleanup or provider process-tree supervision.

## Diagnostics and offline verification

Preflight rejects invalid/incomplete context before opening a supported provider.
A retrieval or validation failure emits a generic advisory diagnostic with a safe error code
and **no additionalContext**. The provider may continue: this is not enforcement.
Callback stdin is capped at 32 KiB/3 seconds; local context reads have a 5-second
timeout and bounded subprocess output. Provider probes have 5-second limits.

After native exit, SwapCtx reports contexts prepared, completed output writes,
failure observations, unfinished callbacks and repeated snapshot identities, or
warns that no callback was recorded. A preparation record establishes only that
the local hook reached validation; it is written before stdout. Failed writes
are diagnosed as `hook_output_failed`, and partial provider receipt is unknown.
Even successful stdout completion cannot establish that the provider accepted
the context, placed it in a model request, or that the model followed it. No
callback can mean unsupported hooks, skipped trust, policy, missing executables,
provider errors or another environment-specific failure.

[Phase 9 continuity](PHASE9_CONTEXT_CONTINUITY.md) adds bounded phase diagnostics
and per-launch keyed fingerprints. Identical snapshots are counted once as a
distinct diagnostic identity, but **context is never suppressed by a prior
preparation or write**. Every callback still reads and validates current memory.
Diagnostic capacity or contention produces an incomplete-observations warning
without preventing an otherwise valid refresh. No receipt/adherence acknowledgement
is inferred from these local records.

```powershell
# Full deterministic gate: no provider/model/soak execution.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 `
  -DeterministicOnly -ReleaseTests

# Targeted suite; requires an already-built CLI and Node.js 24+.
node .\scripts\check-native.cjs .\target\release\swapctx.exe
```

The suite covers real read-only context retrieval, all configured synthetic
lifecycle sources, changed requirements, nested scopes, invalid packets and
events, sensitive/conflicting requirements, timeouts, UTF-8, unsupported versions,
missing callbacks, fake provider errors, signals, cleanup, and the generated
Windows PowerShell command with spaces/apostrophes. A fake native process reads
the generated Claude plugin and exercises a synthetic callback. These establish
the local adapter contract only. Real installed-provider UI compatibility,
Codex session-layer merge/trust behavior, Claude exec-form hook handling, authentic
model receipt/adherence and clean-machine packaging remain separate gates.

The historical Phase 8 Windows offline gate passed formatting, Clippy with warnings denied,
release build, 47 Rust tests in each of debug/release, and 140 supplemental
checks, including 22 native adapter checks and 19 packaging/installer checks.
One opt-in performance test per Rust profile remained ignored. The intentional
hook-input timeout test emits a diagnostic on stderr; it is an asserted failure
case, not a failed suite. See the
[verification record](evidence/phase8-native-offline-2026-10-18.json) for counts,
binary/log hashes, the exact command and outstanding verification boundaries.
This is a development release build; a new static-runtime portable build receipt
and distribution have not been produced for Phase 8.

## Changed files

| File | Purpose |
| --- | --- |
| `src/main.rs` | Optional bare selector, native subcommand and truthful capabilities |
| `src/native.rs` | Native executable discovery, explicit opt-in and embedded adapter lifetime |
| `integrations/native/native.cjs` | Shared validation, provider configuration, bounded hooks, outcome diagnostics and cleanup |
| `scripts/check-native.cjs` | Deterministic offline fixtures and fake-process tests |
| `scripts/verify.ps1` | Include native contract tests in deterministic verification |
| `scripts/prepare-preview.ps1` | Include native tests in preview gates |
| `scripts/release-inputs.cjs` | Inventory the new embedded adapter in isolated builds |
| `scripts/package-windows.ps1` | Permit its bounded source path in integrity receipts |
| `README.md` | Discoverable native commands and verification limits |
| `docs/WINDOWS_RELEASE.md` | Recipient-facing native launch/dependency guidance |
| `docs/PHASE8_NATIVE_PROVIDERS.md` | Setup, official interface evidence, behavior and limitations |
| `docs/evidence/phase8-native-offline-2026-10-18.json` | Local verification evidence |

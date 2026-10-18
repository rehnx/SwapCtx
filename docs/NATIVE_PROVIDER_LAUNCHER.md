# Native provider selector — development implementation

This implementation adds a deliberately small selector to the existing Rust CLI.
There is no replacement chat application, provider SDK, proxy, credential handling,
new service, global configuration change or project-file modification.

## Commands

- `swapctx` — displays **1 Codex / 2 Claude Code / q Quit** when both input and
  output are attached to an interactive terminal.
- `swapctx launch codex` — starts the installed native Codex terminal interface.
- `swapctx launch claude` — starts the installed native Claude Code interface.
- `swapctx launch codex --executable 'C:\\absolute\\path\\to\\codex.cmd'` —
  explicit executable override (also works for Claude).
- Existing `swapctx codex run`, `swapctx context`, memory, protection, import,
  backup and other commands remain unchanged.

Provider discovery searches **absolute PATH entries**, not the current working
folder, and accepts `.exe`, `.cmd` and `.bat` on Windows. The child receives the
current working directory, native terminal handles, user environment and its
own normal authentication/configuration. SwapCtx waits and forwards exit status
in the existing CLI's `u8` exit-code range (other/terminated status maps to 1).
Choosing a real provider may start an authenticated session that consumes model tokens;
this is never done by the offline tests.
If a provider is missing, the launcher prints an installation/PATH/override hint.
Selection without a TTY fails rather than accidentally starting a provider
in a script. Quit returns success without launching anything.

## Integration boundary — important

**The native launch mode is a transparent terminal handoff only.** It does not
silently inject SwapCtx memory into Codex/Claude, read agent conversations,
intercept tool calls, persist agent transcripts, apply file-operation blocks,
or provide native context expansion. The existing **managed** `swapctx codex run`
continues to offer its separate context-aware console; it is *not* the provider's
original terminal UI.

Automatic native-context delivery requires provider-specific supported hooks or
another explicitly validated API. Adding undisclosed prompts to provider command
lines, replacing native model instructions, editing `AGENTS.md`/`CLAUDE.md`, or
pretending filesystem monitoring guarantees enforcement is out of scope.

## Offline verification (only synthetic providers)

Run the ordinary Rust tests from the repository root:

```powershell
cargo fmt
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo test --locked --test provider_launch
```

`src/provider.rs` contains pure menu/path tests and a local fake-child test.
`tests/provider_launch.rs` tests both CLI routes against a temporary fake
`.cmd` on Windows or local shell fixture on Unix, verifies propagated exit
status and output, and checks no unrequested SQLite store is created. No
real Codex/Claude executable is launched and no model/token/soak tests occur.

The code has **not** been validated here with the Windows MSVC compiler or
an actual provider UI. Those are distinct follow-up integration checks and
must never be represented as passed from static review alone.

## Future provider adapter phase (separate scope)

Use a documented native hook/API to deliver safe, read-only, scoped context;
check packet protocol, project identity, freshness and mandatory coverage;
retain the provider's standard UI and user-owned configuration; fail
transparently if hooks are unavailable, and never misrepresent provider receipt
or agent adherence. Keep inference-based validation opt-in and separate.

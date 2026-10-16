# Codex integration readiness

Historical milestone: lifecycle conclusions below are superseded by [Phase 3 managed integration evidence](PHASE3_CODEX_LIFECYCLE.md). The context-first exec launcher remains available.

Verified locally on Windows, 2026-10-12, with Rust 1.99.0, bundled SQLite 3.53.2, Node 24.17.0 and **Codex CLI 0.162.1**. Existing schema 3 and locked dependencies are unchanged. This milestone introduces a [versioned context contract](CONTEXT_PROTOCOL.md), a tested opt-in context-first exec launcher, and an experimental SessionStart bridge. No user Codex configuration, credentials, sessions, hooks or project files were installed or modified by verification.

## Supported surfaces and boundaries

Official Codex documentation describes lifecycle context hooks, including SessionStart after compaction; trusted hooks can supply additional context. It also documents incomplete tool coverage, failure cases that continue execution, and an unsupported `ask` tool-hook decision. Hook definitions require review/trust, and changed definitions require renewed review. These are available design surfaces, not demonstrated SwapCtx recovery or enforcement. [Official hooks documentation](https://learn.chatgpt.com/docs/hooks).

Local MCP servers are a supported tool-delivery option; a registered tool does not ensure the model will invoke it at the right time. SwapCtx does not yet implement an MCP server. [Official MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli).

The app-server provides an initialization handshake, thread/turn methods, typed activity notifications and version-matched schema generation. Its stdio protocol omits the JSON-RPC version header. Events describe observable runtime activity, not hidden model state. SwapCtx does not yet run a persistent event collector. [Official app-server documentation](https://learn.chatgpt.com/docs/app-server).

Codex exec accepts stdin prompts and emits JSON activity events. This is the narrow, working handoff selected after lifecycle tests failed. [Official non-interactive documentation](https://learn.chatgpt.com/docs/non-interactive-mode).

| Capability | Evidence/status |
| --- | --- |
| Scoped versioned context retrieval | Implemented, independent parser/schema and real CLI tests |
| Requirement coverage, provenance, expiry/conflicts, byte limits | Implemented; exclusion and partial coverage fixtures pass |
| Context-first Codex exec handoff | Actual Codex outbound provider input contains selected requirement |
| Launch refusal for stale mandatory requirements | Actual launcher exits before starting Codex/provider requests |
| Structured activity observation | Actual app-server handshake and exec completion events observed; no background collector |
| SessionStart automatic context delivery | **Not demonstrated**: hook discovered but absent from tested exec/app-server provider input |
| Automatic compaction recovery, resume continuity | Unverified; synthetic adapter callbacks are not runtime lifecycle proof |
| Tool blocking / deny, ask, audit integration | Not implemented or verified; existing SwapCtx policies remain manual checks |
| Automatic memory capture, truth/conflict resolution | Not implemented; preserve human-controlled writes |
| Native context expansion / hidden state / universal enforcement | Not supported |

## Reproduction and evidence

From the crate directory, with Node 24+ and an absolute installed Codex path:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 `
  -Offline -ReleaseTests -Benchmark -ContextTests -CodexPath 'C:\absolute\codex.exe'
# Or independently:
node scripts/check-context.cjs target/release/swapctx.exe
node scripts/check-codex.cjs 'C:\absolute\codex.exe' target/release/swapctx.exe
```

The runtime harness is intentionally pinned to Codex 0.162.1 because it asserts a version-specific limitation. Updating Codex requires revalidating that negative fixture rather than silently accepting an assumed behavior. The harness uses isolated temporary CODEX_HOME directories, sanitized environments without API keys, a read-only Codex sandbox and a loopback HTTP Responses fixture. Fixture responses contain no tool calls and no inference. Custom providers are documented by OpenAI. [Official provider configuration](https://learn.chatgpt.com/docs/config-file/config-advanced), [authentication](https://learn.chatgpt.com/docs/auth).

Five real-runtime checks passed: app-server initialization/hook discovery, absent untrusted-hook context, absent context even with the isolated fixture's pre-vetted hook trust bypass, successful stdin delivery with sensitive memory withheld and completed-turn events, and stale mandatory context refusing launch. The hook bypass is used **only** in the isolated negative test; it does not disable the model-tool sandbox. Normal installation must use Codex's hook review flow. No hook installation instructions are promoted while lifecycle delivery remains unverified.

The initial hook delivery acceptance test failed. Both exec (ephemeral and persisted) and app-server startup probes failed to put the marker in provider input. The bridge itself passes synthetic startup/resume/clear/compact fixtures, but this does not establish why the actual runtime omitted it. No upstream cause is asserted. The tested fallback is explicit exec stdin; hooks remain experimental and cannot be relied on to stop or recover an actual session.

Final verification: 47 existing native tests passed in both debug and release; formatting and Clippy with warnings denied passed. 19 new context/bridge checks passed in debug and release, including independent JSON/schema parsing, Unicode/control escaping, literal queries, lifecycle/secret omissions, nested project isolation, budget/record limits, read-only legacy behavior, malformed/oversized/invalid UTF-8 inputs, missing provenance, 64-bit IDs and 5,001-candidate truncation. Ten supplemental SQL migration checks passed with Node SQLite 3.53.0; native tests use the separately bundled engine.

The 100,000-record release fixture retains provenance/events and 50,000 conflicting keys. Twenty complete processes per operation measured context absent-query p50/p95 **834/848 ms** and default context **994/1,119 ms**. Existing operations also passed their benchmark. Context reads include full integrity validation; these measurements are machine-specific observations, not latency guarantees. Current adapters allow five seconds for retrieval and fail on timeout. Future caching/index changes must preserve snapshot integrity and invalidate on writes.

## Explicit launcher use

The optional launcher requires Node, absolute paths to trusted executables/scripts/storage/project, and a nonempty UTF-8 stdin prompt (maximum 16 KiB, 30-second input deadline). It requests an 8 KiB context packet, refuses incomplete mandatory coverage, starts Codex exec with read-only sandbox/ephemeral/JSON options, and passes context and prompt through stdin without shell interpolation. It does not modify configuration or intercept tools. Codex uses its own normal configuration and authentication. Verify capabilities when upgrading Codex.

```powershell
'Explain the current architecture. Do not edit files.' | node `
  'C:\path\to\SwapCtx\integrations\codex\exec-context.cjs' `
  --swapctx 'C:\path\to\SwapCtx\target\release\swapctx.exe' `
  --codex 'C:\path\to\codex.exe' --data-dir 'C:\path\to\SwapCtx-data' `
  --project 'C:\path\to\registered-project'
```

Launching explicitly forwards selected context, root/provenance/rules and your prompt to **the configured Codex provider**. SwapCtx retrieval itself has no network client. Codex configuration, logging, authentication, and hook behavior are outside SwapCtx's control; a read-only model sandbox is not a sandbox for trusted hook scripts. Stored text remains untrusted data and can contain malicious instructions. Delimiting it does not guarantee model behavior. Secret detection is heuristic; review the context packet before first use. Database and backups remain plaintext with inherited local permissions.

## Readiness decision

**Ready for the first constrained, opt-in context-first Codex exec integration and its packaging work. Not cleared for automatic session/compaction recovery, persistent activity capture, or policy enforcement.** Next implement a reviewable installer/preflight and version compatibility checks for this demonstrated route; resolve and reproduce lifecycle delivery with the actual supported runtime before enabling it. Require real-provider opt-in acceptance tests for behavior and compaction recovery, without treating model compliance as guaranteed. Online dependency advisories and hosted Windows/Linux CI remain release gates; they could not be executed in this restricted local environment. Do not advertise general production release clearance until those gates pass.

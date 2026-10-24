# Support and troubleshooting

The public release is [v0.1.0-preview for Windows x64](https://github.com/rehnx/SwapCtx/releases/tag/v0.1.0-preview).
Native Codex and Claude Code sharing remains experimental. Linux CI does not
establish Windows behavior, and no Linux/macOS binaries or stable support period
are provided. Start with these resources:

- **Installation**: [Windows release guide](docs/WINDOWS_RELEASE.md)
- **Commands & Memory CLI**: [CLI usage guide](docs/USAGE.md)
- **Native AI Adapters**: [Native provider setup guide](docs/PHASE8_NATIVE_PROVIDERS.md)
- **Contributor Setup**: [Contributor guide](CONTRIBUTING.md)
- **All Guides and Evidence**: [Documentation index](docs/README.md)

General bug reports and feature ideas can be submitted via the repository's [issue tracker](https://github.com/rehnx/SwapCtx/issues).
Use [the issue chooser](https://github.com/rehnx/SwapCtx/issues/new/choose) for a
bug report or improvement proposal. Report vulnerabilities privately through
[SECURITY.md](SECURITY.md), even if discovered while troubleshooting.
*Note: As an open-source project, no dedicated SLA, response-time commitment, or hosted support service is provided.*

---

## Collect safe diagnostics

For CMD command lookup, use `where.exe swapctx`; PowerShell uses `Get-Command
swapctx -CommandType Application`. If lookup fails, invoke the installed
`swapctx.exe` by absolute path or follow the session PATH instructions in the
[Windows guide](docs/WINDOWS_RELEASE.md#executable-discovery-and-path).

New builds use ASCII automatically on unknown legacy consoles. To request it
explicitly, run `set "NO_COLOR=1"` in CMD or `$env:NO_COLOR='1'` in PowerShell
before starting SwapCtx. Include both the shell (CMD, PowerShell 5.1 or 7) and
the host (Windows Terminal or legacy Windows Console) in rendering bug reports.

Run these commands from your project folder to diagnose issues locally without launching external providers or transmitting data:

```powershell
Get-Command swapctx -CommandType Application
swapctx --version
swapctx capabilities
swapctx status
swapctx doctor
```

When filing a report, please include:
1. SwapCtx version, installation method, and source revision or binary SHA-256
2. Windows OS version
3. Exact command, arguments, and exit code
4. Expected vs. actual behavior with a minimal synthetic reproduction

> **Privacy Warning:** Redact account names, private file paths, passwords, and project content. Do not upload SQLite databases, agent conversation transcripts, provider config files, credentials, or environment dumps. Follow [SECURITY.md](SECURITY.md) for sensitive findings.

---

## Common problems & solutions

| Symptom | Cause / Check | Solution |
| --- | --- | --- |
| `swapctx` is not found | PATH environment variable does not include SwapCtx installation folder. | Add installation directory (e.g., `%LOCALAPPDATA%\SwapCtx-0.1.0-preview`) to your User Path in Windows environment variables, then restart terminal. |
| Project not registered | Calling command outside a registered project directory. | Run `swapctx init` at project root. Nested registered projects have separate memory scopes in the same store. Ensure consistent `--data-dir` if overriding store locations. |
| Node/provider executable missing | Core memory needs no Node. Native context adapters need Node.js 24+ and installed providers. Plain native EXEs need no Node; npm JavaScript entries do. | Verify the relevant runtime and provider installation. For `swapctx native`, use `--executable` or `--node` to specify explicit paths if required. |
| Compatibility warning / plain UI fallback | Provider version/help probe did not pass context hook requirements. | Verify provider CLI supports required hook capability flags. A plain native session can launch without SwapCtx context sharing; the provider retains its own authentication, permissions and network behavior. |
| No callback recorded | Hooks untrusted, disabled, or skipped by provider. | Check provider configuration and diagnostic logs. Successful preflight output does not imply provider hook invocation. |
| Context withheld | Incomplete requirement coverage, conflicting records, or secret detection triggered. | Run `swapctx context` locally to inspect reported coverage and exclusions. Address conflicting or invalid records explicitly. |
| Database busy | Another process holds a SQLite lock. Writable connections wait up to 30 seconds per busy operation; read-only connections keep five seconds. Caller deadlines can be shorter. | Close parallel writing tools and retry deliberately. The wait is not a whole-command deadline. Do not delete SQLite WAL/SHM sidecar files or reset the store. |
| Integrity error | Corrupted or unrecognized SQLite store format. | Preserve the store directory. Restore a verified backup into a new directory using `swapctx --data-dir '...' restore <backup.db>`. Do not run `init` as repair. |
| Installer destination exists | Destination folder already exists during install. | Specify a new, non-existent destination directory for `install-windows.ps1`. |

---

Persistent PATH changes, provider trust decisions, and live-provider debugging are user-controlled actions. Routine project verification uses offline fixtures only.

# Support and troubleshooting

Welcome to SwapCtx Support! If you encounter issues while running SwapCtx or setting up integrations, start with the following resources:

- **Installation**: [Windows release guide](docs/WINDOWS_RELEASE.md)
- **Commands & Memory CLI**: [CLI usage guide](docs/USAGE.md)
- **Native AI Adapters**: [Native provider setup guide](docs/PHASE8_NATIVE_PROVIDERS.md)
- **Contributor Setup**: [Contributor guide](CONTRIBUTING.md)

General bug reports and feature ideas can be submitted via the repository's [issue tracker](https://github.com/rehnx/SwapCtx/issues).
*Note: As an open-source project, no dedicated SLA, response-time commitment, or hosted support service is provided.*

---

## Collect safe diagnostics

Run these commands from your project folder to diagnose issues locally without launching external providers or transmitting data:

```powershell
Get-Command swapctx -CommandType Application
swapctx --version
swapctx capabilities
swapctx status
swapctx doctor
```

When filing a report, please include:
1. Source revision or binary SHA-256 fingerprint
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
| Node/provider executable missing | Core memory needs no Node. Native adapters need Node.js 24+ and installed provider executables. | Verify Node.js 24+ (`node -v`) and provider installation. For `swapctx native`, use `--executable` or `--node` flags to specify explicit paths if required. |
| Compatibility warning / plain UI fallback | Provider version/help probe did not pass context hook requirements. | Verify provider CLI supports required hook capability flags. A plain native session can launch without SwapCtx context sharing; the provider retains its own authentication, permissions and network behavior. |
| No callback recorded | Hooks untrusted, disabled, or skipped by provider. | Check provider configuration and diagnostic logs. Successful preflight output does not imply provider hook invocation. |
| Context withheld | Incomplete requirement coverage, conflicting records, or secret detection triggered. | Run `swapctx context` locally to inspect reported coverage and exclusions. Address conflicting or invalid records explicitly. |
| Database busy | Database locked by another active process writing to SQLite. | Close parallel writing tools and retry. Do not delete SQLite WAL/SHM sidecar files. |
| Integrity error | Corrupted or unrecognized SQLite store format. | Preserve the store directory. Restore a verified backup into a new directory using `swapctx --data-dir '...' restore <backup.db>`. Do not run `init` as repair. |
| Installer destination exists | Destination folder already exists during install. | Specify a new, non-existent destination directory for `install-windows.ps1`. |

---

Persistent PATH changes, provider trust decisions, and live-provider debugging are user-controlled actions. Routine project verification uses offline fixtures only.

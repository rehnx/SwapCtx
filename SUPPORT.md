# Support and troubleshooting

Start with [installation](docs/WINDOWS_RELEASE.md), [CLI usage](docs/USAGE.md) and
[native provider setup](docs/PHASE8_NATIVE_PROVIDERS.md). General bug reports can
use the repository's [issue tracker](https://github.com/rehnx/SwapCtx/issues).
No hosted support service or response-time commitment is offered.

## Collect safe diagnostics

From the affected project, these commands stay local and do not launch providers:

```powershell
Get-Command swapctx -CommandType Application
swapctx --version
swapctx capabilities
swapctx status
swapctx doctor
```

Report the source revision or binary SHA-256, Windows version, exact command and
exit code, expected behavior and a synthetic reproduction. Redact account names,
private paths and any saved text. Do not upload databases, transcripts, provider
configuration, credentials or environment dumps. Follow [SECURITY.md](SECURITY.md)
for sensitive findings.

## Common problems

| Symptom | Check |
| --- | --- |
| `swapctx` is not found | Add its installation directory to the current process PATH or your user Path; reopen the terminal and verify `Get-Command`. |
| Project not registered | Run `init` at the intended project root. A nested registered project has separate memory. Use the same `--data-dir` on every command when overriding storage. |
| Node/provider executable missing | Core memory needs neither. Optional adapters need Node 24+; native mode requires underlying `.exe` files on Windows. Use absolute overrides from the native guide. |
| Compatibility warning and plain UI | The version/help probe did not establish the required hook interface. Plain launching is preserved; context was not enabled. |
| No callback recorded | Hooks may be untrusted, disabled, unsupported or skipped. Inspect provider diagnostics. Do not infer delivery from preflight preparation. |
| Context withheld | Review memory freshness, conflicts, provenance and mandatory coverage locally. Do not bypass validation to force stale or incomplete requirements into the provider. |
| Database busy | Close other writers and retry. Back up before maintenance; do not remove SQLite sidecars. |
| Integrity error | Preserve the entire store. Restore a verified backup into a new explicit directory; `init` is not corruption repair. |
| Installer destination exists | Choose a new directory. Installation deliberately never merges with an existing installation or data store. |

Persistent PATH changes, provider trust decisions and live-provider debugging are
user-controlled actions. Routine project verification uses offline fixtures only.

# Install SwapCtx on Windows

The public [v0.1.0-preview release](https://github.com/rehnx/SwapCtx/releases/tag/v0.1.0-preview)
supports **Windows x64**. The binary is unsigned. There are no official WinGet,
Scoop, Linux, or macOS packages. Core memory commands need no Node.js, Rust,
compiler, or AI provider at runtime.

## Download and verify

Download these three assets into the same directory:

- [swapctx-0.1.0-preview-windows-x64.zip](https://github.com/rehnx/SwapCtx/releases/download/v0.1.0-preview/swapctx-0.1.0-preview-windows-x64.zip)
- [install-windows.ps1](https://github.com/rehnx/SwapCtx/releases/download/v0.1.0-preview/install-windows.ps1)
- [SHA256SUMS.txt](https://github.com/rehnx/SwapCtx/releases/download/v0.1.0-preview/SHA256SUMS.txt)

Open Windows PowerShell in that download directory. Check both files before
running the installer:

```powershell
$ExpectedZipSha256 = '42df6ee920ac158e7c3b49cc1902267054973d5fee404fcd659269bb2ede31c4'
$ExpectedInstallerSha256 = '4002d49f29715653f4df9f9a333584fb6719b819f73b6a4c6a7a111a981619b2'
if ((Get-FileHash -LiteralPath .\swapctx-0.1.0-preview-windows-x64.zip -Algorithm SHA256).Hash -ine $ExpectedZipSha256) { throw 'ZIP checksum mismatch' }
if ((Get-FileHash -LiteralPath .\install-windows.ps1 -Algorithm SHA256).Hash -ine $ExpectedInstallerSha256) { throw 'Installer checksum mismatch' }
```

These values identify the existing published assets, not later source builds.
Compare with the official release through a channel you trust. Checksums detect
changed bytes; they do not independently authenticate the publisher. Inside the
ZIP, `manifest.json` lists each payload file's SHA-256 and length.

## Install

Install or extract the ZIP into a new local folder. No administrator privileges,
PATH modification, telemetry, or background service is required. Core memory
commands need only swapctx.exe. The optional managed console additionally needs
Node.js 24+ and Codex CLI 0.162.1; it sends selected context to your configured
Codex provider when you explicitly run it.

The binary also supports `swapctx native codex` and `swapctx native claude`,
which launch the installed provider's original UI. Bare `swapctx` opens a provider
selector in an interactive terminal. Memory exposure is off by default. Optional
`--context` requires Node.js 24+ and a provider advertising the required hook or
plugin interface; older versions fall back to ordinary native launching. This
adapter has synthetic test coverage, with live compatibility and native
receipt/adherence unverified. See [native setup](PHASE8_NATIVE_PROVIDERS.md) for
limitations. No provider is bundled or authenticated by SwapCtx.

After both checks above pass, install into a **new** directory under the existing
local application-data directory:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install-windows.ps1 `
  -Archive .\swapctx-0.1.0-preview-windows-x64.zip `
  -ExpectedSha256 $ExpectedZipSha256 `
  -Destination "$env:LOCALAPPDATA\SwapCtx-0.1.0-preview"
if ($LASTEXITCODE -ne 0) { throw 'Installation failed' }

$SwapCtx = Join-Path $env:LOCALAPPDATA 'SwapCtx-0.1.0-preview\swapctx.exe'
& $SwapCtx --version
# Expected: swapctx 0.1.0-preview
```

The parent directory must exist and the destination must be new. The script
checks the archive hash and every payload hash, rejects unexpected entries,
then installs by a directory rename. It never executes package contents, changes
PATH, downloads anything or modifies a store. It requires only Windows
PowerShell 5.1; Node and Rust are not installation dependencies. The process-only
execution-policy flag does not authenticate downloaded code. Manual extraction
remains available, with manual hash verification your responsibility.

## Start using memory

Use an absolute executable path, especially when changing project directories:

```powershell
# $SwapCtx is the absolute executable path set during installation above.
& $SwapCtx --version
& $SwapCtx --help
Set-Location 'C:\Projects\YourProject'
& $SwapCtx init
& $SwapCtx memory add --kind requirement --key compatibility 'Preserve existing public interfaces'
& $SwapCtx memory search 'interfaces'
& $SwapCtx context --max-bytes 8192
& $SwapCtx doctor
```

To use the bare command from any project in your current PowerShell session:

```powershell
$InstallDirectory = Join-Path $env:LOCALAPPDATA 'SwapCtx-0.1.0-preview'
$env:PATH = "$InstallDirectory;$env:PATH"
Get-Command swapctx -CommandType Application
swapctx --version
Set-Location 'C:\Projects\YourProject'
swapctx status
```

Check that `Get-Command` resolves the expected executable. This PATH change ends
with the terminal session. It does not move your data or change the working
directory. The installer prints the equivalent command for your chosen folder.
For persistent command lookup, use Windows' **Edit environment variables for your
account**, edit your **user** Path, and add this installation directory as one
entry. Keep existing entries. Close terminal applications and launch a new one
from Start, then check `Get-Command swapctx -CommandType Application` again.
This manual Windows step needs no development tools or administrator privileges.
It has not been performed on the current user's profile by our tests. Avoid
adding directories containing semicolons to PATH. Automatic persistent PATH
registration and WinGet/Scoop installation are not provided; no public package
identifier has been assigned or submitted.

## Optional recipient acceptance

The release has three downloadable assets; it does **not** attach the producer's
acceptance script or JSON reports. Ordinary installation does not require them.
For a maintainer or recipient repeating the offline acceptance procedure, obtain
`scripts/test-windows-preview.ps1` from the exact
[published source revision](https://github.com/rehnx/SwapCtx/blob/962e1e649851684fab897b96f714e493f1e1c969/scripts/test-windows-preview.ps1)
or the original maintainer packet. Git checkouts/downloads can differ in line
endings; verify the exact script bytes before execution instead of assuming an
arbitrary download matches the packet. The script requires Windows PowerShell
5.1. Check its own SHA-256 against the trusted distribution channel before
running it. It verifies the ZIP and installer hashes, installs into disposable
storage, removes developer tools from its process PATH, and tests core memory,
project discovery, boundary exit codes, and backup/recovery. It never starts
the optional Codex adapter. Work and report paths must be new, with existing
parents; the report must be outside the disposable work directory.

```powershell
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass `
  -File .\test-windows-preview.ps1 `
  -Archive .\swapctx-0.1.0-preview-windows-x64.zip -ExpectedSha256 'ZIP_HASH' `
  -Installer .\install-windows.ps1 -InstallerSha256 'INSTALLER_HASH' `
  -WorkDirectory "$env:TEMP\SwapCtx-preview-work" `
  -Report "$env:USERPROFILE\Desktop\SwapCtx-preview-report.json"
```

Use the full 64-digit published hashes, not the placeholders. This test restores its process
environment, removes its disposable work directory, and preserves an existing
report instead of replacing it. The resulting JSON describes the machine and
PowerShell versions and lists checks passed. A pass on a development laptop with
PATH restricted still does not equal a pass on a clean Windows installation.
The [release verification record](RELEASE_VERIFICATION.md) distinguishes the
owner's reported separate-machine pass from publicly inspectable reports.

`init` registers the current directory. Nested directories use the nearest
registered ancestor. Default data lives in `%LOCALAPPDATA%\SwapCtx`, outside the
executable directory. Use `--data-dir 'C:\YourLocalData\SwapCtx'` on **every**
command if choosing another store. No project contents are scanned automatically.
Memory, backups and exports are plaintext; keep them in a suitably private folder.
Secret detection is heuristic. Context retrieval is external memory, not native
model context expansion, guaranteed adherence, or file protection.

## Native provider interfaces

With a separately installed provider, keep its original terminal UI:

```powershell
& $SwapCtx native codex
& $SwapCtx native claude
# Explicitly expose selected project memory for this invocation:
& $SwapCtx native codex --context
& $SwapCtx native claude --context
```

Plain native `.exe` launches share no SwapCtx context and need no Node.
A standard npm JavaScript entry also needs Node when context sharing is off.
Context mode requires Node.js 24+ and the documented hook capabilities.
Windows native discovery accepts a native `.exe` or resolves a standard provider
npm shim to its validated package entry without executing the shell shim.
Use `--executable ABSOLUTE_PATH` to select either supported installation and
`--node ABSOLUTE_PATH` when a Node runtime is needed. Custom shell layouts are
refused by `native`; the separate plain `swapctx launch` command supports trusted
shell shims. Native setup never modifies provider settings or instruction files.

An unsupported probe warns and opens the ordinary UI without context. Invalid
or incomplete mandatory context fails closed. Hooks remain subject to provider
trust/policy. Each permitted startup/resume/clear/compact/fork callback reads
fresh memory; SwapCtx cannot establish that every provider emits those callbacks.
Preparation and completed output are local observations, not proof of receipt,
retention or model adherence. No exact native provider release is certified.

## Optional managed Codex console

```powershell
& $SwapCtx codex setup --codex 'C:\Tools\Codex\codex.exe'
& $SwapCtx codex status
& $SwapCtx codex run --observe
# A second terminal in the same registered project:
& $SwapCtx codex watch
```

`status` and `watch` are local-only. `run` starts Codex and can consume model
tokens. `doctor` under the codex subcommand also starts Codex. Read-only is the
console default. To deliberately allow workspace edits, repeat setup with
`--workspace-write --replace`. Ordinary Codex launches do not activate SwapCtx.
Use `/help` inside the console. Context refresh is before turns; observation is
not enforcement. Natural automatic-compaction recovery remains unverified.

## Upgrade and recovery

Close running SwapCtx consoles before upgrading. Retain the old executable and
back up your data **with the old executable before any migration**:

```powershell
& $SwapCtx backup 'C:\Backups\swapctx-before-upgrade.db'
```

The destination parent must exist; existing backups are never overwritten.
Extract an upgrade to a new folder and change `$SwapCtx` to its executable.
Do not delete your data folder or SQLite WAL/journal sidecars. Reads accept
recognized legacy schemas; writes may migrate them transactionally. Older
executables may reject upgraded stores. Downgrade by retaining the original
store or restoring a compatible backup into a **new** directory:

```powershell
& $SwapCtx --data-dir 'C:\Recovery\NewSwapCtxStore' restore 'C:\Backups\swapctx-before-upgrade.db'
& $SwapCtx --data-dir 'C:\Recovery\NewSwapCtxStore' doctor
```

The new directory must not exist and its parent must exist. This does not replace
the current store. Use its `--data-dir` explicitly for subsequent commands.
Backups cover saved project memory and rules, not native Codex conversation
history or adapter session/settings files. To uninstall the portable program,
close it and remove its executable folder; data remains separately stored.
If you added a user Path entry, remove only that installation's entry in Windows
environment-variable settings and open a new terminal. Keep your data directory
and backups unless you separately intend to delete that plaintext data.

## Diagnostics and release limits

`swapctx capabilities`, `swapctx privacy`, and `swapctx --help` are local.
`swapctx doctor` checks stored data without repairing it. On failure, preserve
the store and inspect the diagnostic; do not reset data as a repair strategy.
Exit codes: 0 success, 1 operation error, 2 usage error or deny rule, 3 ask rule.
Boundary rules report policy matches; they never intercept file operations.

This candidate includes Apache-2.0 LICENSE, Cargo.lock, full collected dependency
notice texts and a machine-readable inventory of their source hashes. These
cover locked Windows Cargo dependencies, including build dependencies, and
additional Unicode and bundled SQLite notices. This is not legal clearance or
an inventory of separately installed Node, Codex, Rust or MSVC runtime components.
The public-candidate build recipe links the C runtime statically and rejects
dynamic CRT imports. Ordinary `cargo build --release` output can still import
VCRUNTIME140.dll; use the candidate produced by the documented release recipe.
The [published release evidence](RELEASE_VERIFICATION.md) records the exact source,
asset hashes, CI, and acceptance-report limitations. A hosted pass does not
establish clean personal-machine compatibility. Hashes do not establish
source-to-binary provenance. No real-model evaluation is required or performed by packaging.

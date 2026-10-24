# Install SwapCtx on Windows

SwapCtx supports Windows x64 without administrator privileges. Core memory
commands need no Node.js, Rust, compiler or AI provider at runtime. The binary
is unsigned. There are no official WinGet or Scoop packages.

## Download and verify

New source-built candidate packets include the ZIP, **install-windows.cmd**,
**install-windows.ps1**, and **SHA256SUMS.txt**. Keep both installer files in the
same directory. CMD is the default setup entry point; Windows PowerShell 5.1
provides the installation engine. PowerShell 5.1 and PowerShell 7 can also invoke
the PowerShell installer directly. Neither entry point switches your running shell.

The existing [v0.1.0-preview release](https://github.com/rehnx/SwapCtx/releases/tag/v0.1.0-preview)
has the original ZIP, PowerShell installer and checksums only. The CMD wrapper
and terminal fixes described here require a new source build/candidate; they
have not been added to those published assets. For that original packet, use
the PowerShell commands below with its published checksums. Historical identities
are recorded in [release verification](RELEASE_VERIFICATION.md).

Obtain the packet and expected hashes through a channel you trust. Before
executing either installer, verify **the ZIP and both installer files** against
that packet's `SHA256SUMS.txt`. From CMD:

```bat
certutil -hashfile "swapctx-0.1.0-preview-windows-x64.zip" SHA256
certutil -hashfile "install-windows.cmd" SHA256
certutil -hashfile "install-windows.ps1" SHA256
```

Compare every resulting hash with its trusted checksum entry and stop on any
mismatch. In PowerShell 5.1 or 7, these commands produce the same SHA-256 values:

```powershell
Get-FileHash -LiteralPath .\swapctx-0.1.0-preview-windows-x64.zip -Algorithm SHA256
Get-FileHash -LiteralPath .\install-windows.cmd -Algorithm SHA256
Get-FileHash -LiteralPath .\install-windows.ps1 -Algorithm SHA256
```

Checksums detect changed bytes; they do not independently authenticate a
publisher. Do not run an unverified installer to verify itself. The installer
independently checks the expected ZIP hash and every payload hash and length.

## Install from CMD (default)

Open CMD in the download directory. Replace `ZIP_SHA256_FROM_TRUSTED_CHECKSUMS`
with the ZIP's 64 hexadecimal hash characters, after the checks above pass:

```bat
install-windows.cmd -Archive ".\swapctx-0.1.0-preview-windows-x64.zip" -ExpectedSha256 ZIP_SHA256_FROM_TRUSTED_CHECKSUMS -Destination "%LOCALAPPDATA%\SwapCtx-0.1.0-preview"
if errorlevel 1 echo Installation failed; stop and inspect the error.
```

The parent directory must exist and the destination must be **new**. Repeating
an installation into the same directory fails and preserves its contents. Use
a new versioned directory for upgrades. Paths containing spaces and Unicode are
supported. In CMD, use double quotes and keep delayed expansion off (the default);
CMD expands percent variables in commands you type. From a batch script use
`call install-windows.cmd ...` if that script must continue afterwards.

The CMD wrapper uses the OS Windows PowerShell host with profiles and prompting
disabled and returns its exit code. It runs the adjacent `.ps1` file without
interpreting its contents as a shell command. The PowerShell engine snapshots the
archive, verifies hashes, rejects unexpected/traversal/link entries, and installs
by directory rename. Neither entry point runs package contents, downloads files,
modifies PATH or user data, or requires elevation. The process-only execution
policy flag does not authenticate code or change persistent policy.

## Install from PowerShell 5.1 or 7

Use the same trusted ZIP hash and a new destination:

```powershell
$ExpectedZipSha256 = 'ZIP_SHA256_FROM_TRUSTED_CHECKSUMS'
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File .\install-windows.ps1 `
  -Archive .\swapctx-0.1.0-preview-windows-x64.zip `
  -ExpectedSha256 $ExpectedZipSha256 `
  -Destination "$env:LOCALAPPDATA\SwapCtx-0.1.0-preview"
if ($LASTEXITCODE -ne 0) { throw 'Installation failed' }
```

For PowerShell 7 use `pwsh -NoProfile -NonInteractive -File` with the same
arguments. You can also invoke `.\install-windows.cmd` from either PowerShell.
Manual ZIP extraction remains available with manual verification of the payload
hashes and lengths in `manifest.json` your responsibility.

## Executable discovery and PATH

First verify the installed executable by absolute path. In CMD:

```bat
"%LOCALAPPDATA%\SwapCtx-0.1.0-preview\swapctx.exe" --version
set "PATH=%LOCALAPPDATA%\SwapCtx-0.1.0-preview;%PATH%"
where.exe swapctx
swapctx --version
```

In PowerShell 5.1 or 7:

```powershell
$InstallDirectory = Join-Path $env:LOCALAPPDATA 'SwapCtx-0.1.0-preview'
$SwapCtx = Join-Path $InstallDirectory 'swapctx.exe'
& $SwapCtx --version
$env:PATH = "$InstallDirectory;$env:PATH"
Get-Command swapctx -CommandType Application
swapctx --version
```

Confirm discovery resolves the intended installation. PowerShell requires `&`
before a quoted executable path; CMD invokes the quoted path directly. PATH
changes above affect only the current session. The installer prints commands for
both shells. Paths containing semicolons cannot be a single PATH entry; use an
absolute executable path instead. For CMD session setup, avoid percent signs and
exclamation marks in the installation directory as well.

For persistent lookup, open Windows **Edit environment variables for your
account**, edit your **user** Path, and add only the installation directory as
one entry. Keep existing entries, close terminal applications, and launch a new
one from Start. Verify discovery again. No administrator privileges or system
PATH changes are required; the installer never performs this step automatically.

## Start using memory and the native menu

After session PATH setup, these commands work in CMD and both PowerShell versions:

```bat
cd "C:\Projects\YourProject"
swapctx init
swapctx memory add --kind requirement --key compatibility "Preserve existing public interfaces"
swapctx memory search "interfaces"
swapctx context --max-bytes 8192
swapctx doctor
swapctx
```

Bare `swapctx` opens the menu in the current console and accepts a number or
`q`/Enter. It does not replace CMD or PowerShell. Native commands inherit your
terminal handles. Provider discovery uses absolute PATH entries and excludes the
project directory; it does not depend on PowerShell aliases or profiles.

Windows Terminal uses ANSI styling when virtual terminal processing can be
enabled on the actual console handle. Rust writes Unicode to console handles
independently of CMD's OEM code page. Unknown legacy console fonts cannot be
reliably probed, so those hosts get ASCII automatically. `NO_COLOR` (including
an empty value) or `TERM=dumb` selects the ASCII UI. Unicode paths in that menu
are displayed as escaped code points on limited consoles; their actual paths,
stored memory and JSON remain unchanged. SwapCtx never changes the code page or
font. To select ASCII explicitly, set `NO_COLOR=1` in CMD or `$env:NO_COLOR='1'`
in PowerShell. Redirection disables styling; the menu refuses noninteractive
input/output and starts no provider. Use explicit native commands in scripts.

If command lookup fails, check `where.exe swapctx` in CMD or `Get-Command swapctx
-CommandType Application` in PowerShell, reopen the terminal after a user Path
change, or invoke the executable by absolute path. Keep your data directory and
backups when changing installations. For rendering problems, use `NO_COLOR=1`
and report the Windows version and terminal host, separately from the shell.

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

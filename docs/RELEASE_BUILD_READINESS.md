# Windows build and command discovery readiness

This pass addresses a concrete portability risk, tests release compilation in
separate source locations, and verifies the bare `swapctx` command across projects.
It preserves existing CLI commands, data schemas, context protocol, portable ZIP
layout and installer behavior. Existing development build/package commands remain
available. No public release or package-manager submission was made.

## Discovered and implemented

The original `cargo build --release` executable imports VCRUNTIME140.dll and
several dynamic Universal CRT APIs. Copying it around this development laptop
does not prove a clean recipient has those runtime components. The new explicit
release recipe builds x64 MSVC output with `+crt-static`, `/Brepro`, no debug
information and stable Rust source-prefix remapping. It sets a fixed source date
epoch and disables Cargo incremental compilation. Bundled SQLite also receives
the compiler's `/Brepro` flag. An explicit Cargo target keeps target-specific
runtime flags separate from host procedural macro compilation.

`scripts/build-windows.cjs` copies known source inputs to two separate source
directories and compiles to two fresh target directories. It refuses to publish
output unless the resulting executable SHA-256 hashes match and PE inspection
finds no dynamic CRT import. The optional `--test` runs all native release tests
under the same static-runtime build settings. The resulting directory contains
swapctx.exe and a build-info.json receipt with compiler identity, source hashes,
recipe hash, independent binary hashes, test flag and direct/delay DLL imports.
The receipt includes no raw local source paths, timestamps or Git/user identity.

`scripts/inspect-pe.cjs` reads PE data without loading or executing the image.
Inspection of the new output lists only these DLLs, with no delay imports:

- API-MS-WIN-CORE-SYNCH-L1-2-0.DLL
- BCRYPTPRIMITIVES.DLL
- KERNEL32.DLL
- NTDLL.DLL

This removes the observed VCRUNTIME dependency; it does not prove compatibility
with every Windows version or all transitively loaded system libraries.

The packager's optional `-BuildInfo` checks the receipt against the copied binary,
current source hashes and current recipe, and ships it beside the ZIP with its
own checksum. Existing ZIP/manifest v1 and installer compatibility are preserved;
receipts are not inserted into the ZIP. Packaging also refuses payloads beyond
the installer's decoded file/total size limits. Existing callers without a
receipt remain supported, including development binaries with dynamic CRT imports.
Use the receipt-backed static build for public candidates.

Git attributes pin LF for compiled Rust/SQL and embedded JavaScript inputs, as
well as Cargo metadata and the recipe, to prevent checkout newline conversion
from silently changing compiled input bytes.

## Build and package offline

Prerequisites: pinned Rust 1.99.0 Windows MSVC toolchain in the ordinary user
rustup location, MSVC C++ build tools/Windows SDK, Node.js 24+, and previously
cached locked Cargo dependencies. Recipient machines do not need these build
tools. This script uses CARGO_HOME if explicitly set; otherwise this project's
`.build/cargo-home` cache. For a standard Cargo cache, set it explicitly as below.
It rejects compiler-wrapper/CL/LINK overrides rather than pretending those builds
are controlled. Other native toolchain/configuration differences are not pinned.

```powershell
New-Item -ItemType Directory -Force .\.build | Out-Null
# When dependencies are in the normal Cargo cache instead of .build/cargo-home:
# $env:CARGO_HOME = Join-Path $env:USERPROFILE '.cargo'
node .\scripts\build-windows.cjs .\.build\windows-release --test
node .\scripts\check-release.cjs .\.build\windows-release\swapctx.exe --static
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\package-windows.ps1 `
  -Binary .\.build\windows-release\swapctx.exe `
  -BuildInfo .\.build\windows-release\build-info.json `
  -Destination .\.build\windows-preview
```

Both output directories must be new, with existing parents. Downloads are
disabled in the build recipe. Failure cleans owned staging; forced termination
may leave it. Source changes during copying fail. The script does not copy
databases, credentials or repository metadata. Output is an unsigned local
candidate. A receipt is a consistency record, not authenticated provenance: an
attacker who can replace the binary and receipt can fabricate matching evidence.
Metadata and test receipts are not security attestations.

## Invoke from any project

After verified installation, enable its directory for this terminal session:

```powershell
$InstallDirectory = 'C:\Users\YourName\Tools\SwapCtx-0.1.0'
$env:PATH = "$InstallDirectory;$env:PATH"
Get-Command swapctx -CommandType Application
swapctx --version
Set-Location 'C:\Projects\YourProject'
swapctx init
swapctx memory add --kind requirement 'Preserve existing interfaces'
Set-Location .\src
swapctx context
```

The installer prints the session command with your actual destination escaped
for PowerShell. No registry, machine PATH, profile or persistent environment
settings are changed. Bare-command tests verify two separately registered
projects and nested directories, a program folder with spaces, and the documented
PowerShell command. Test child processes use disposable data and session PATH.
No actual user PATH/store was changed. Use an installation path without a
semicolon, since semicolons delimit PATH entries.

Persistent registration and standard package-manager installation remain future
distribution work. No WinGet/Scoop identifier, hosted download URL or installation
command is advertised as working. A verified public URL and finalized publisher
identity must precede manifests; nothing was submitted here.

## Evidence and preview decision

Two isolated builds produced the same SHA-256 on this machine, and all 47 native
release tests passed with static-runtime settings. The normal debug/release
foundation checks and the static executable's context, session, everyday,
packaging, command discovery and receipt checks also passed. Exact commands and
counts are retained in the [offline evidence](evidence/release-build-offline-2026-10-17.json).
No models, AI agents, inference
or soak evaluations ran.

This proves same-machine repeatability with the same cached dependencies,
compiler, MSVC/SDK installation and environment. Cross-machine, cross-SDK,
cross-toolchain and cross-cache-location reproducibility remain unverified;
the native linker/SDK and complete inherited build environment are not pinned
or fully inventoried. Default Cargo development binaries remain dynamic-runtime
builds. Hosted CI has the PE/command checks but has not run here; the independent
static release recipe still requires its documented explicit verification.

The local candidate is ready for independent clean-machine preview acceptance.
It is not yet cleared for public publication: current dependency advisory review,
human redistribution review (including static compiler runtime obligations),
hosted CI and clean Windows installation/downloaded-file testing remain open.
Code signing, authenticated provenance and standard package-manager distribution
are absent. The previously demonstrated temporary-cwd PowerShell access failure
also still requires checking outside this managed environment. These gaps do not
justify additional model evaluations; they are distribution/environment checks.

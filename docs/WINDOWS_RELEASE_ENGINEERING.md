# Portable Windows release engineering

This report records the first candidate. The subsequent
[preview distribution hardening](PREVIEW_DISTRIBUTION.md) adds verified local
installation, a collected dependency notice bundle, and additional rejection
tests. Read it for the current package layout and release boundaries.

This maintenance pass prioritizes the gap between a tested source checkout and
a usable distribution. Existing retrieval, storage schema, context protocol,
commands and adapter behavior are unchanged.

## Implemented

- Corrected Cargo's MIT declaration to Apache-2.0, matching the existing LICENSE;
  the license text itself was not replaced.
- Added `scripts/package-windows.ps1`. It accepts a trusted, already-built binary,
  verifies the copied executable's x64 PE header and reported package version,
  and packages only five explicitly named files plus a generated manifest.
  No data directory, environment file, log, credential or historical evaluation
  report is collected. The executable already embeds its Node adapter modules.
- Added SHA-256 hashes and byte lengths for payload files, plus a separate ZIP
  checksum. Stable entry ordering and timestamps avoid incidental ZIP changes
  on the same packaging runtime. This is not reproducible compilation or a
  guarantee of byte equality across .NET/compressor versions.
- Stages output beside the requested destination and publishes the ZIP/checksum
  directory by a no-replacement directory move. Existing destinations fail;
  ordinary errors clean up owned staging. A killed process can leave staging.
- Added a self-contained quickstart covering installation, local memory use,
  optional console dependencies, backup-before-upgrade, new-directory recovery,
  uninstall, privacy, exit codes and release limitations.
- Added six offline package checks to deterministic Windows verification and
  Windows CI. Tests unpack to temporary paths containing spaces and exercise
  the distributed executable, including embedded local-only adapter status with
  a deliberately nonexistent Codex executable.

## Commands

From the source folder with cached build dependencies:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -DeterministicOnly -ReleaseTests
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\package-windows.ps1 -Destination .\.build\windows-candidate
```

The destination must not exist; its parent must exist. Packaging itself does not
build, run the full suite, or certify the binary. A matching version cannot prove
that a binary came from the current source. Use the just-verified release binary.
No installer, automatic updater or PATH modification was introduced.

## Failure investigation

The first package test exposed a Windows .NET ZIP restriction: entry timestamps
cannot be changed after contents have been written in Create mode. The packager
now sets each timestamp before opening the entry stream. The failing attempt
cleaned staging and published no candidate. Subsequent package tests passed.

Running the documented default-binary command then exposed a PowerShell 5.1
parameter-default scope issue: PSScriptRoot was empty while evaluating that
default. Default resolution now happens in the script body. A sixth regression
test runs the exact default-binary path from the release binary directory instead
of the source root. This restricted environment refused PowerShell with a
temporary directory as its working directory, so that particular cwd remains
unverified; archive extraction and CLI data operations still use temporary paths.

The initial full verification log recorded successful suites, but an outer
PowerShell redirection treated normal Cargo stderr output as a NativeCommandError.
Verification was repeated using direct child-process exit-status capture to
remove that ambiguity; the final result is recorded in the evidence file.

## Release boundary

The archive is an unsigned local candidate. Checksums detect accidental changes;
they do not authenticate a publisher, prove source provenance, or defend against
someone changing both files and hashes. Package only a trusted executable:
the version check executes it. Publication and code signing were not performed.

Public-release clearance still needs hosted CI, clean-machine validation,
dependency advisory review and redistribution-license review. The included
dependency inventory is not a complete notice/license bundle or legal clearance.
No external services, real models, Codex processes or soak evaluations were
used. No additional model behavior or automatic recovery has been demonstrated.

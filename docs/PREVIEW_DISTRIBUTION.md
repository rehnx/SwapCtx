# Preview distribution hardening

For the later static-runtime build comparison and bare-command verification,
see [release build readiness](RELEASE_BUILD_READINESS.md). This report describes
the preceding notice/installer milestone; the remaining external release gates
still apply.

The first candidate demonstrated working portable extraction. This pass closes
two practical gaps: shipping dependency notices and verifying installation before
running the downloaded executable. Storage schema, context protocol, command
syntax, adapter behavior and user stores are unchanged. Nothing was published.

## Verified local installation

The package output now contains a ZIP, `install-windows.ps1`, and SHA256SUMS.txt
covering both. The ZIP includes the executable, existing documentation/license
files, THIRD_PARTY_NOTICES.txt, third-party-inventory.json and manifest.json.

The installer requires a SHA-256 value supplied by the caller and a **new**
destination directory. It copies the input to private staging, verifies its
hash, then validates the complete flat ZIP entry list, manifest version/target,
unique names, file lengths and SHA-256 hashes. It rejects links, directories,
unknown names (including traversal paths), duplicate entries, missing files and
unsupported manifests. Archive and total decoded payload are bounded at 64 MiB;
each file at 32 MiB and the manifest at 128 KiB. Extraction bounds actual decoded
bytes as well as declared lengths. Only verified payloads are published through
a directory rename that refuses existing destinations.

The installer executes no package content, starts no agent, downloads nothing,
does not edit PATH, and never opens or migrates a user database. Staging is
cleaned on ordinary failure; termination can leave owned staging directories.
Local hostile-process races, hardware power loss and hostile filesystem drivers
are outside the guarantees. Expected hashes and the installer itself must come
from a trusted channel: changing files and checksums together defeats checksum
verification. This is not code signing or publisher authentication.

See WINDOWS_RELEASE.md for end-user commands; that file becomes QUICKSTART.md in
the package. Manual extraction remains supported. Installing an upgrade into a
new directory preserves the previous program; back up data before running an
upgrade that could migrate a store.

## Reproducible dependency notices

`scripts/third-party.cjs` uses offline locked Cargo metadata for the Windows x64
target. It verifies cached `.crate` archive SHA-256 values against Cargo.lock,
then reads original notice text directly from those archives without extraction.
It includes all root LICENSE/COPYING/NOTICE/UNLICENSE files, the nested
regex-syntax Unicode data license, and the first public-domain notice from the
bundled SQLite amalgamation. All packages in the generated Windows inventory,
including build dependencies, are covered. Both alternative root licenses are
retained where present; this does not choose a licensing branch for recipients.

The generated inventory records crate checksums, original notice-file hashes,
the current Cargo.lock hash and generated notice bundle hash. It contains no
local source paths. The packager refuses mismatched lock/notice hashes;
deterministic verification also regenerates and compares exact output. Git
attributes pin LF for these hashed source files to avoid checkout conversion.

With the pinned compiler and cached dependencies available on PATH/CARGO_HOME:

```powershell
node .\scripts\third-party.cjs --check
# After an intentional dependency change, generate and review the diff:
node .\scripts\third-party.cjs --write
```

The generator fails rather than downloading missing archives. Initial inspection
found no per-file checksum metadata in this local extracted cache; generation
therefore uses the actual archive hashes in Cargo.lock. GNU long-name tar records
were present and are handled without filesystem extraction. Unrecognized archive
entry types fail for review. This is an inventory and notice-collection check,
not legal clearance, an advisory scan, or proof of build provenance. Rust/MSVC
runtime components and separately installed Node/Codex are outside the inventory.

## Verification and environment limits

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -DeterministicOnly -ReleaseTests
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\package-windows.ps1 -Destination .\.build\preview-candidate
```

The package destination must be new and its parent must exist. Twelve Windows
package checks cover allowlisted hashes, installed bytes, nested retrieval,
backup/restore, embedded local-only status, unchanged existing destinations,
wrong hashes, changed payloads, invalid executables, duplicate manifests/ZIP
entries, traversal names and malformed archives. Same-input ZIP bytes match on
this packaging runtime; compilation and cross-runtime ZIP reproducibility are
not established. The packager still requires a trusted, already-verified binary
and executes only its `--version` check; the installer never executes it.
An isolated source-copy test also verifies that changed notice text is rejected
before packaging. A ZIP mutation fixture initially missed an explicit .NET
compression assembly import; the fixture was corrected and the suite rerun.

The earlier temporary-cwd packaging failure persists, but a reduced probe also
fails with bare PowerShell `Write-Output OK`, `Get-Location`, and current-directory
inspection from that same temporary location. An absolute-path .NET operation
works there. Thus the failure is not specific to packaging; the underlying access
restriction remains unidentified. Packaging and installation work from source
and release directories while reading/writing disposable temporary paths.
Temporary-cwd PowerShell startup needs verification outside this environment.

No Codex process, real model, external AI agent, soak evaluation, network audit,
publication, package submission or remote-repository mutation was performed.

## Public preview decision

Distribution is materially stronger and locally testable without Node or Rust
on the recipient's machine. Remaining public-release gates are hosted CI,
clean-machine Windows/PowerShell testing (including runtime availability and
downloaded-file handling), current dependency advisory review, and human
redistribution review of collected notices and compiler/runtime obligations.
Signing and source-to-binary provenance are not implemented. No installer
behavior under SmartScreen/Mark-of-the-Web has been verified here. The native
model-behavior and automatic-compaction limitations remain unchanged.

Publication requires explicit user approval after the concrete candidate and
remaining gates have been reviewed. This turn creates local candidates only.

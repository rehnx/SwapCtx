# Preparing a Windows preview

This is the current local producer procedure. Earlier phase reports are retained
as historical evidence; they do not certify newer source or binaries. The
[Phase 11 assessment](PHASE11_RELEASE_READINESS.md) records the latest candidate
and distinguishes local results from external release gates.

## Prerequisites

- Windows x64, Windows PowerShell 5.1, Node.js 24+.
- Pinned Rust 1.99.0 with rustfmt/Clippy, MSVC C++ Build Tools and Windows SDK.
- All locked dependencies cached locally. Provisioning tools/dependencies is a
  separate action that may require network access; the producer itself is offline.
- A new destination under an existing parent. Do not overwrite prior candidates.
- Trusted build environment/executables; no unreviewed compiler/linker overrides.

## One producer command

From the crate root, once the prerequisites are available:

```powershell
New-Item -ItemType Directory -Force .\.build | Out-Null
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\prepare-preview.ps1 `
  -Destination .\.build\phase11-preview
```

If that directory already exists, choose a new name such as `phase11-preview-2`.
The script refuses existing destinations and cleans only owned staging. Handled
failures leave no candidate destination; a forced kill can leave staging to inspect.

The workflow:

1. Runs `verify.ps1 -DeterministicOnly -ReleaseTests`: formatting, lint, tests,
   locked offline builds, documentation checks and synthetic integration fixtures.
2. Builds in two isolated source/target directories with static CRT, path remapping
   and deterministic linker settings; compares executable hashes. Runs native
   Rust tests on the release recipe and rejects dynamic CRT imports.
3. Independently validates the copied binary, complete source inventory, build
   helper hashes and release-test receipt. Embedded native/managed adapter sources
   are part of the code inventory.
4. Creates the portable ZIP, exact manifest, dependency notices and checksums;
   reruns adapter/package/PE/receipt regressions on the static binary.
5. Runs PowerShell-only recipient acceptance against the actual ZIP and installer,
   using disposable data and a restricted process PATH, then finalizes the new
   local directory only after all gates pass.

Fresh compile trees use an owned system-temp directory to avoid MSVC output-path
limits in deeply nested checkouts (including this OneDrive workspace). Final
assembly stays beside the destination, so publication does not depend on a
cross-volume rename. Both temporary trees are cleaned with ownership checks;
cleanup failures retain an originating build error. A short, trusted Windows TEMP
location is still a build prerequisite if the account's TEMP itself is deeply nested.

The compiler runs from canonical Windows paths. Source, Cargo-cache and generated
output paths remap both ordinary/8.3 and device spellings to fixed virtual roots.
The producer logs executable hashes before/after native tests. On a mismatch it
reports bounded PE section hashes and difference offsets before cleanup; it keeps
the complete-file equality gate and publishes no candidate. No binary fields are
ignored or rewritten to force a match.

This starts no real providers and performs no inference, downloads, uploads,
publication, persistent PATH changes or access to existing user memory. Offline
pilot-oracle fixtures are evaluator unit checks, not model evaluations. Existing
optional benchmark, model, pilot and soak modes are excluded.

## Candidate contents and trust

The output directory contains the ZIP, installer, recipient acceptance script,
`SHA256SUMS.txt`, `build-info.json`, `release-checks.json` and
`windows-acceptance.json`. The ZIP contains `swapctx.exe`, Apache-2.0 `LICENSE`,
`QUICKSTART.md`, `DEPENDENCIES.md`, `Cargo.lock`, generated third-party notices,
their inventory and `manifest.json`. It includes no working database, credentials
or conversation history.

The build receipt inventories Rust/SQL/adapter code and build tools, not all
repository documentation. Phase 11 evidence separately hashes presentation and
release documents. Local unsigned receipts and same-machine matching builds do
not establish authenticated source-to-binary provenance or cross-machine
reproducibility. Checksums must be obtained from a trusted channel.

To inspect an already prepared candidate without modifying it:

```powershell
Get-Content .\.build\phase11-preview\SHA256SUMS.txt
Get-Content .\.build\phase11-preview\release-checks.json
Get-Content .\.build\phase11-preview\windows-acceptance.json
```

Installation, session/user PATH setup, upgrade and uninstall are documented in
the packaged [Windows quickstart](WINDOWS_RELEASE.md). Core recipient acceptance
needs no Rust, Node, compiler or agent. A restricted PATH on a development machine
still does not demonstrate clean-machine compatibility.

## Release decision

Before any public distribution, the maintainer must review:

- The exact candidate hashes, complete test results, source inventory and notices.
- Current dependency advisories from a fresh database; an offline cache without
  evidence of freshness does not clear current vulnerability risk.
- Hosted verification for the candidate revision and existing or future independent
  clean-machine evidence. Do not label local acceptance as either of these.
- A verified private security-reporting route and supported preview scope.
- Publisher identity: sign the final binary/installer, or explicitly approve an
  unsigned preview with documented hash distribution and expected Windows warnings.
  No signing certificate purchase or service enrollment is part of this procedure.
- Native integration labeling: offline contracts are tested; actual provider
  compatibility, callback scheduling, receipt and adherence remain unverified.

## Private hosted candidate retention

The manual `windows-preview.yml` workflow can retain its fully tested output for
maintainer review. By default `retain_candidate` is false and nothing is uploaded.
When explicitly enabled during an authorized development run, it first requires a
private repository, then retains the candidate as a private Actions artifact for
seven days. The artifact name includes the workflow source SHA. The producer and
recipient acceptance must both pass before retention. A public repository cannot
use this retention path; public distribution needs a separately approved release
workflow.

Download from that exact run, inspect the receipts and checksums, and preserve the
source SHA with the artifact. This is internal candidate review, not publisher
authentication or independent-machine acceptance. Do not copy an artifact to a
public download or publish it as a release without founder approval.

Follow the authorization granted for the current task when dispatching builds or
pushing source. Tags, visibility changes, external distribution, package submissions
and public releases remain separate launch actions; this procedure grants none.

# Phase 6: local preview preparation and approval handoff

This pass closes consistency gaps between release evidence and the candidate
actually handed to testers. It also supplies a recipient-side acceptance script
that needs no Rust, Node, compiler or AI coding agent. Stored data, schemas, CLI
syntax, context protocol and the portable ZIP/installer v1 layout are preserved.
The optional Node/Codex adapter still has its documented separate dependencies.

## Implementation

Receipt-backed packaging now regenerates the complete sorted source inventory,
checks its fingerprint, checks hashes of all build recipe helpers, requires the
receipt's native release-test flag, and independently inspects the copied PE
executable. False DLL-import claims and partial nonempty source inventories can
no longer be accepted merely because selected receipt fields match. Receipts
remain unsigned local consistency records, not authenticated provenance. A new
receipt from the current build recipe is required; old receipts are preserved
but refused as current-release evidence. Non-receipt development packaging is
still supported. Node is needed for producer receipt validation, not installation.

The shared source enumerator removes duplicate inventory definitions. The build
recipe now honors RUSTUP_HOME and falls back to the ordinary user Cargo cache
when neither CARGO_HOME nor a workspace cache is available. It still checks the
pinned compiler, builds offline, compares two fresh source/target directories,
and rejects direct/delayed dynamic CRT imports. The resulting executable matched
the prior static binary's hash; these are release-tooling changes.

`test-windows-preview.ps1` is shipped beside the ZIP and installer, covered by
SHA256SUMS.txt. It first verifies supplied ZIP and installer hashes, installs to
new disposable storage, checks the installed executable against its manifest,
and limits its process PATH to the installed program and Windows tools. It
isolates LOCALAPPDATA and exercises sixteen assertions:

- archive, installer, installed binary and version consistency;
- absence of Node/Cargo/rustc/Codex applications on PATH, and bare-command lookup;
- informational commands without implicit database creation;
- default-store creation, nested project discovery and separation of projects;
- manual boundary deny exit code (no enforcement claim);
- backup overwrite refusal, new-store restore, existing-store refusal and doctor.

The script restores its process environment and location, removes owned working
files, and writes a new JSON report without replacing existing reports. Expected
command failures are checked by exit code. Hash mismatch stops before installer
execution. It never runs a Codex command, accesses actual user stores or mutates
persistent PATH. The reports contain OS/PowerShell versions and check names, not
project contents, credentials or raw tool transcripts. Same-machine runs retain
Windows components already installed; the report explicitly states this limit.

## One producer command

From the source folder with Node 24+, pinned Rust/MSVC/SDK and cached dependencies:

```powershell
New-Item -ItemType Directory -Force .\.build | Out-Null
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass `
  -File .\scripts\prepare-preview.ps1 -Destination .\.build\approved-candidate
```

The destination must be new and its parent must exist. This runs the deterministic
foundation suite, creates/test-compares two independent static release builds,
validates the receipt against current source and the copied executable, runs the
static binary's context/session/everyday/package/PE/receipt tests, and runs the
PowerShell-only acceptance test against the actual staged distribution. It
publishes locally by a no-replacement directory rename only after all gates pass.
There is no network, release, upload, package submission, commit or push step.
Ordinary failure cleans owned staging and leaves the destination absent.

For an explicitly trusted, already independently built/tested current receipt:

```powershell
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass `
  -File .\scripts\prepare-preview.ps1 -Destination .\.build\review-candidate `
  -ExistingBuild .\.build\phase6-release
```

This avoids recompiling the same two builds while retaining all subsequent
receipt, foundation, binary and recipient checks. Reuse is recorded in
release-checks.json, not misrepresented as a fresh build. Input receipts must
describe current source/tool hashes and successful native release tests.

Output includes the ZIP, installer, acceptance script, receipt, recipient report,
gate summary and SHA256SUMS.txt. Source/compiler logs are not shipped. Checksum
verification does not establish publisher identity; a trusted channel is still
required. The fresh-build and trusted-reuse branches share the build recipe,
but the combined local pipeline was exercised using trusted reuse after a
separate successful two-build/native-test run. Its default fresh orchestration
still needs a hosted run.

The manual `.github/workflows/windows-preview.yml` is prepared locally. It
provisions the toolchain/cache in a separate network step, then invokes the
offline producer command and displays its reports. It has read-only repository
permissions and no upload/publication step. It has not been pushed or dispatched.

## Installing and invoking globally

Recipients can use the hash-checked installer with Windows PowerShell alone;
see WINDOWS_RELEASE.md (QUICKSTART.md in the ZIP). Session PATH is checked locally
with bare `swapctx` commands across nested, separate projects. For persistence,
the quickstart describes adding the installed folder to Windows' user Path via
the normal environment-variable UI, keeping existing entries and verifying a
fresh terminal. This needs no development tools. No current-user registry/PATH
change was made by these tests. Persistent registration must be exercised in
the independent-user acceptance gate; it is not claimed as locally demonstrated.
No working WinGet/Scoop identifier or download URL has been invented.

## Verification and approval actions

The retained [offline evidence](evidence/phase6-release-offline-2026-10-17.json)
records exact runs, hashes and limitations. All testing is deterministic offline;
no models, agents, inference or soak evaluations were run. No new evidence of
model behavior, enforcement or automatic context recovery is claimed.

Before public-release approval:

1. Choose the supported Windows x64 versions and run the shipped acceptance script
   on clean installations/accounts. Verify the trusted downloaded artifacts,
   SmartScreen/Mark-of-the-Web behavior, user PATH in a newly launched terminal,
   ordinary install/uninstall and retention of separately stored data. The local
   developer-path restriction is useful evidence, not a substitute for this.
2. Approve/push the source and manually run hosted Windows preparation plus the
   existing foundation/advisory workflows. Complete a current advisory scan;
   no up-to-date advisory database was consulted in this offline environment.
3. Review dependency notices and static compiler/runtime redistribution obligations;
   finalize publisher identity, trusted download channel, unsigned-preview/signing
   policy and exact release hashes. Signing and authenticated provenance are absent.
4. Approve the exact candidate for publication. Package-manager manifests require
   that approved identity and immutable hosted artifacts; none were submitted.

The candidate is prepared for independent acceptance, not cleared for public
publication. Remaining cross-machine/toolchain reproducibility, downloaded-file,
temporary-cwd PowerShell and licensing/advisory gates are explicitly open.

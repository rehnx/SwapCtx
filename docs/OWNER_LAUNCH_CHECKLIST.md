# v0.1.0-preview owner launch checklist

New features are frozen. This procedure does not authorize publication.
Engineering supplies exact source/tree identities, CI/producer URLs, artifact
identity/expiry, hashes and receipts. Never substitute a later rebuild.

## Private security intake and publisher trust (#5)

The preview is intentionally unsigned: no Authenticode or signed build attestation.
The founder must accept that disclosed trust level before publication. After launch
approval, publish approved hashes in release notes and SHA256SUMS through the
founder-controlled GitHub repository over HTTPS. This still trusts the GitHub
account/build environment; adjacent checksums are not publisher authentication.

GitHub documents private vulnerability reporting for **public** repositories:
[official instructions](https://docs.github.com/en/code-security/security-advisories/working-with-repository-security-advisories/configuring-private-vulnerability-reporting-for-a-repository).
The owner reports that the repository is public, private reporting is enabled,
and the private report form was checked without submitting a report. The public
[advisories page](https://github.com/rehnx/SwapCtx/security/advisories) exposes
**Report a vulnerability**. PR #21 records the corrected reporting instructions.
Keep any vulnerability details private; do not submit a fake vulnerability.

The owner authorizes an unsigned preview with founder-controlled HTTPS hashes,
provided the final-source CI, exact candidate and separate-host Windows gates
pass. This conditional authorization does not turn an earlier candidate's
results into evidence for a newer source revision.

## Mainline protection

The owner reports configured branch protection. Recheck the current settings and
required checks through GitHub before merging: require pull requests, current-base
checks, and the applicable Foundation jobs; block force pushes and deletion.
A public push/PR intentionally skips `windows-recipient`; the final manual run on
`main` must run that job successfully. An expected skip is not a failed check.
If the only merge blocker is a solo owner's inability to approve their own PR,
the owner authorizes reducing required approving reviews to zero while preserving
all other protections. Do not bypass failed checks or broadly disable protection.

## Independent Windows acceptance (#6)

Use a separate Windows x64 machine or clean VM with Windows PowerShell 5.1,
preferably without Rust, Node, MSVC or providers. Same-machine hosted acceptance
with a restricted PATH does not establish independent-machine compatibility.
No independent physical Windows host/credentials are attached to the cloud
environment. Private CI and an explicit public manual Foundation run on `main` transfer the exact tested package from its
producer to a separate Windows 2022 runner and preserves that recipient report.
When passed, this supplies independent hosted-VM/OS evidence. It does not prove
behavior on a clean personal machine without developer tools installed.

Transfer the exact candidate privately through a founder-approved route. Preserve
the seven original files: ZIP, installer, acceptance script, checksums, build
receipt, release checks and hosted acceptance report. Verify ZIP, installer and
acceptance-script hashes against the final release packet before running scripts.
Keep the hosted report and create a separate independent report. Repacking the
ZIP changes its identity.

Use the exact command/hashes in the final packet. Its parameter form is:

```powershell
$ExpectedZipSha256 = Read-Host 'Paste approved ZIP SHA-256'
$ExpectedInstallerSha256 = Read-Host 'Paste approved installer SHA-256'
Get-FileHash .\test-windows-preview.ps1 -Algorithm SHA256
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass `
  -File .\test-windows-preview.ps1 `
  -Archive .\swapctx-0.1.0-preview-windows-x64.zip `
  -ExpectedSha256 $ExpectedZipSha256 `
  -Installer .\install-windows.ps1 `
  -InstallerSha256 $ExpectedInstallerSha256 `
  -WorkDirectory .\independent-acceptance-work `
  -Report .\independent-windows-acceptance.json
if ($LASTEXITCODE -ne 0) { throw 'Independent acceptance failed' }
```

Both outputs must be new, with writable parents. Review the script first;
process-only execution-policy bypass does not establish trust. It uses disposable
data and no provider. Record exit status, report, machine classification,
Windows/PowerShell versions and exact source/asset identities in #6. Redact
usernames/private paths; investigate failures instead of editing reports.
Persistent user PATH setup remains a separate manual Windows check.

## Final engineering evidence and prerelease preparation

- Select merged source with clean working tree and all Foundation jobs passed:
  Windows/Linux, advisories, history scan, fresh Windows producer and the separate
  Windows recipient (private CI or public manual run on `main`).
- Verify strict pre/post/independent-build hashes, static CRT, native release
  tests, package/receipt regressions and all recipient checks. Keep #13 scoped
  to actual native toolchain evidence; never normalize differences away.
- Download the authorized Actions artifact before seven-day expiry. Verify artifact digest,
  original checksums, ZIP manifest and source/tool inventories. Archive original
  bytes/logs/receipts privately; receipts remain unsigned local observations.
- Fill the release-note identity block with actual hashes and run URLs. Native
  Codex/Claude remains **experimental, synthetic-tested, live receipt/adherence
  unverified**. Live inference is unnecessary for this preview.
- Prepare prerelease title/tag `v0.1.0-preview` targeting the approved full SHA.
  Keep draft text/assets local while gates remain open; do not create a release
  or tag as an incidental verification step.

## Founder authorization and subsequent launch actions

The owner authorizes these actions after #5/#6/#13 have final evidence or the
explicitly accepted bounded disposition. Verify exact source, hashes and unsigned
disclosures before publication:

1. Verify the repository remains public and private security intake remains usable.
   Review reachable history; secret scanning is heuristic.
2. Create the approved tag and GitHub **prerelease** on that exact revision.
   Leave **Set as latest release** off. Attach unchanged approved assets,
   checksums/receipts and finalized release notes.
3. Verify downloaded hashes against the retained originals. Do not rebuild after
   approval. No package-manager submission or announcement is implied.

Then close owner gates with evidence. No ownership, permissions, secrets, billing
or organization changes are authorized by this checklist.

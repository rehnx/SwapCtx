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
A 404 while private cannot verify its state; this execution token also receives
403 for administrative settings. No reporting channel has been verified.

Before public launch, supply a maintainer-controlled private contact route,
verify receipt with a harmless message, and update SECURITY.md with that confirmed
route. Do not invent an address or submit a fake vulnerability. If visibility and
reporting setup must be coordinated, document a usable temporary intake first.
After authorized visibility change, an administrator opens **Settings → Code
security** (possibly grouped under Code security and analysis), enables **Private
vulnerability reporting**, then checks **Security → Report a vulnerability**
from a non-maintainer account. Record the verified route/date in #5 without
vulnerability details. Keep this gate open until private intake is usable.

## Independent Windows acceptance (#6)

Use a separate Windows x64 machine or clean VM with Windows PowerShell 5.1,
preferably without Rust, Node, MSVC or providers. Same-machine hosted acceptance
with a restricted PATH does not establish independent-machine compatibility.
No independent physical Windows host/credentials are attached to the cloud
environment. Private CI additionally transfers the exact tested package from its
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
  Windows recipient (private CI).
- Verify strict pre/post/independent-build hashes, static CRT, native release
  tests, package/receipt regressions and all recipient checks. Keep #13 scoped
  to actual native toolchain evidence; never normalize differences away.
- Download the private artifact before seven-day expiry. Verify artifact digest,
  original checksums, ZIP manifest and source/tool inventories. Archive original
  bytes/logs/receipts privately; receipts remain unsigned local observations.
- Fill the release-note identity block with actual hashes and run URLs. Native
  Codex/Claude remains **experimental, synthetic-tested, live receipt/adherence
  unverified**. Live inference is unnecessary for this preview.
- Prepare prerelease title/tag `v0.1.0-preview` targeting the approved full SHA.
  Keep draft text/assets local while gates remain open; do not create a release
  or tag as an incidental verification step.

## Founder authorization and subsequent launch actions

After #5/#6/#13 have evidence or an explicitly reviewed bounded disposition,
approve the exact source, hashes, unsigned disclosure and these actions:

1. Change visibility to public. Review private history, branches, issues and PRs
   first; secret scanning is heuristic. Verify private security intake.
2. Create the approved tag and GitHub **prerelease** on that exact revision.
   Leave **Set as latest release** off. Attach unchanged approved assets,
   checksums/receipts and finalized release notes.
3. Verify downloaded hashes against the retained originals. Do not rebuild after
   approval. No package-manager submission or announcement is implied.

Then close owner gates with evidence. No ownership, permissions, secrets, billing
or organization changes are authorized by this checklist.

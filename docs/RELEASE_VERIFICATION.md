# v0.1.0-preview: published asset verification

Verified on 2026-10-23 against the live
[GitHub prerelease](https://github.com/rehnx/SwapCtx/releases/tag/v0.1.0-preview),
its API records, and downloaded public assets. This record describes the existing
release; documentation updates on main do not change its source or binaries.

## Source and distribution

- Tag: [`v0.1.0-preview`](https://github.com/rehnx/SwapCtx/tree/v0.1.0-preview),
  resolved through GitHub's tag-reference API to commit
  [`962e1e649851684fab897b96f714e493f1e1c969`](https://github.com/rehnx/SwapCtx/commit/962e1e649851684fab897b96f714e493f1e1c969).
- GitHub marks this a **prerelease**, not a draft. Public release platform:
  Windows x64. The executable and release evidence are unsigned.
- The three attached assets are the portable ZIP, `install-windows.ps1`, and
  `SHA256SUMS.txt`. No published asset was replaced or rebuilt for this review.

SHA-256 values independently recomputed from fresh public downloads:

**Portable ZIP** — [download](https://github.com/rehnx/SwapCtx/releases/download/v0.1.0-preview/swapctx-0.1.0-preview-windows-x64.zip)

```text
42df6ee920ac158e7c3b49cc1902267054973d5fee404fcd659269bb2ede31c4
```

**Installer** — [download](https://github.com/rehnx/SwapCtx/releases/download/v0.1.0-preview/install-windows.ps1)

```text
4002d49f29715653f4df9f9a333584fb6719b819f73b6a4c6a7a111a981619b2
```

**Executable extracted from the downloaded ZIP**

```text
b53f452486b0fa383771708bade31cf4c2ad82aad656bf5dcbe8d193d656a06d
```

**Published SHA256SUMS.txt** — [download](https://github.com/rehnx/SwapCtx/releases/download/v0.1.0-preview/SHA256SUMS.txt)

```text
c225ce88e466ef68b06eb7ff63558ea8abc0281da6e2095da042085f7ff844f2
```

The downloaded ZIP and installer match the expected release identities, the
published manifest entries, and GitHub's API-reported asset digests. The checksum
file's digest also matches its API record. The extracted executable matches the
expected hash and the ZIP's payload manifest. Payload file hashes and lengths
were checked against `manifest.json`.

`SHA256SUMS.txt` also lists producer files that are **not attached** to the public
release: `test-windows-preview.ps1`, `build-info.json`, `windows-acceptance.json`,
and `release-checks.json`. Their listed hashes do not make those files accessible
or establish their contents. The independent recipient report is a separate
report, not the producer's `windows-acceptance.json`.

These checks establish byte identity against recorded expectations. Hashes are
not signatures and do not independently authenticate the publisher, certify the
source-to-binary relationship, or establish clean-machine compatibility.

## Hosted CI

[Final source verification run](https://github.com/rehnx/SwapCtx/actions/runs/39902646906)
for the published source completed successfully:

- `secret-history`, `advisories`, `test (windows-latest)`,
  `test (ubuntu-latest)`, and `windows-preview` passed.
- `windows-recipient` was intentionally **skipped** for the public repository.
  Do not describe this run as separate-host recipient acceptance.
- The Windows producer runs offline gates and disposable recipient checks on
  its own host. Rust/Node fixtures test core and local adapter contracts;
  they do not establish authenticated native-provider behavior.

Historical reports and earlier artifact hashes remain scoped to their recorded
revision. Same-environment matching builds do not imply cross-image or
cross-toolchain byte reproducibility.

## Independent recipient evidence

The owner's [final acceptance comment in issue #6](https://github.com/rehnx/SwapCtx/issues/6#issuecomment-6358810928)
records the exact final source SHA, ZIP hash, installer hash, a **separate Windows
machine**, the recipient script name, and a passing result. It states that the
script verified installation and exercised core commands with developer tools
excluded from its process PATH. Live Codex/Claude integration was not tested.

That comment refers to sanitized acceptance JSON, but no attachment or report
URL was present in the issue body or its eight comments at this review. No final
independent report was publicly accessible there or among the three release
assets. It has therefore **not been reviewed or republished**. The actual Windows
and PowerShell versions, per-check outcomes, machine cleanliness, and persistent
user PATH/uninstall scope cannot be confirmed from that comment alone.

**Owner follow-up:** attach or link the actual final independent report in #6
after reviewing it for private usernames, absolute personal paths, hostnames,
identifiers, tokens, credentials, project memory, and transcripts. Preserve its
source/asset identities, OS/PowerShell versions, check results, and limitations.
State which fields were redacted. If a result is not in the report, record it as
unverified rather than inferring it from hosted CI or an earlier candidate.

Use the [Windows guide](WINDOWS_RELEASE.md) for installation and
[SECURITY.md](../SECURITY.md) for private findings. Keep the original published
assets, tag, and source commit unchanged.

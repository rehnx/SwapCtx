# Windows binary comparison investigation (#13)

The release gate compares complete files, before/after native tests and across two
fresh source/target directories. Neither timestamps, sections nor byte ranges are
removed or normalized. Failed or untested outputs are never release candidates.

## Measured baseline experiment

Source `2cf69e864756193703e5fc1c427c68357d5f7ddc` (application version 0.1.0,
before the prerelease version update), workflow
[39792472372](https://github.com/rehnx/SwapCtx/actions/runs/39792472372):

| Input/result | Windows latest | Windows 2022 |
| --- | --- | --- |
| Hosted image | win25-vs2026 / 20261007.264.1 | win22 / 20261012.333.1 |
| MSVC toolset | 14.51.36231 | 14.44.35207 |
| SDK directory version | 10.0.26100.0 | 10.0.26100.0 |
| cl.exe SHA-256 | b0a1818ebae6da7f18230a84e76ce839212584ac4c2ab55b97a0d5268085da57 | fe251ef50a1545b1b0835ee17b1e785459712b38d79e45b5c1d3d28970a36619 |
| SDK libucrt.lib SHA-256 | ea2ffa5d9b1ebc3fa76f34c364471750db8951b341dbccbc1bef628ef1a20e39 | 2b247d6a27c22c520c34134b8acf2da3366175a4690886927a51929f37725fdd |
| Executable bytes | 5,147,648 | 5,131,264 |
| Executable SHA-256 | 07ffabf56e369208403b99900ac40c8fee8c0acb64484995bb3797de5d3a9c6a | 48309ad026b3f9eb328824ceae481fa1b46ac8d8138a016c347a7a08551c2dec |

Each environment's four pre/post hashes matched its own executable and receipt.
Downloaded private artifacts 12433095063 and 12431927877 were verified against
their Actions archive digests before inspecting all four raw binaries. Private
retention expires after seven days; local copies preserve the original bytes.

Read-only PE comparison counted 2,486,202 differing byte positions, including
executable `.text`, `.rdata`, `.data`, `.pdata` and relocation sections. The
`.fptable` section matched. Both import the same Windows DLL names and no dynamic
C runtime. This is not a timestamp-only difference. Native compiler/runtime bytes
are different even though Rust and the SDK directory version match. Directory
version labels alone do not identify all native inputs.

This establishes observable native-input and output differences; it does not
isolate one compiler/library change as the sole cause. These baseline binaries
are investigation inputs, not the final `0.1.0-preview` candidate. Later exact
source/asset identities belong in the final release packet.

## Historical failure and disposition limits

Earlier run [39772749108](https://github.com/rehnx/SwapCtx/actions/runs/39772749108)
rejected unequal independent builds on main 880ac24. It cleaned owned staging, so
its raw files cannot be retrospectively compared. Canonical path remapping fixed
a concrete alias gap, but the definitive cause of that historical mismatch is
unproven. Passing later runs do not prove it.

The revised producer explicitly selects native tools and records their binary
hashes and key runtime-library hashes, sets absolute compiler/linker/archiver
paths, preserves opt-in raw diagnostics, and fails immediately on pre/post
mutation. Independent receipt checks reject missing identity and tampered
observations. The original full-byte gate remains in force.

The supported claim is matching fresh builds under the recorded toolchain and
inputs in each verified run. General cross-image/cross-machine reproducibility,
exhaustive SDK/header inventory and signed provenance are unverified. Recurrence
under an identical recorded identity must block candidate selection and reopen
the investigation. Final head/main checks and retained final candidate evidence
are required before closing the engineering gate.

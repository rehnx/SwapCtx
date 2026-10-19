# Phase 11 — Presentation and release readiness

Review date: 2026-10-19. Starting local HEAD: `5679410` (Phase 10); working tree
was clean. **GO for a release decision; NO-GO for public publication until the
gates below are addressed or explicitly accepted.** A fresh local Windows
candidate passed the full offline producer pipeline. Public-release approval
remains pending; no remote operation was performed.

## Presentation and documentation

The README now introduces persistent project memory, native provider selection,
explicit context sharing, installation and a short executable quick start. It
uses a restrained text hero, factual license/platform badges, a real workflow
status link and a Mermaid architecture diagram. The static terminal excerpt
shows SwapCtx's menu, not fabricated provider activity. No raster assets or
provider screenshots were needed; all presentation is source-controlled text.

Historical statuses were reconciled: ordinary native launching remains distinct
from opt-in hooks and the older managed console. Phase 9 continuity and Phase 10
hardening supersede older source status, while historical authenticated evidence
is scoped to the managed console and its specific checks. No exact native
provider release, automatic recovery guarantee or enforcement is claimed.

Detailed usage moved to `docs/USAGE.md`; engineering reports remain intact.
Contributor/security/support/changelog documents and issue/PR templates define
safe offline reproduction and accurate claims. The current producer procedure
is `docs/RELEASE_PREPARATION.md`. No Rust behavior, CLI, schema, dependencies,
provider settings, user stores or licensing text were changed.

## Local verification

The supported command completed with exit code **0**:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\prepare-preview.ps1 `
  -Destination .\.build\phase11-preview
```

No existing build was reused. The new directory contains the ZIP, installer,
acceptance script, checksums, current build receipt, producer gate report and
recipient acceptance report. Previous candidates were not overwritten.

| Verification | Actual result |
| --- | --- |
| Formatting and Clippy | Passed; all targets, warnings denied. |
| Rust tests | 55 passed in each of debug, ordinary release and fresh static-release configurations; one existing performance test ignored per configuration. |
| Foundation supplemental checks | Schema 10, context 23, session 23, everyday 28, native 33, package/installer 19, PE/PATH 4, offline oracle fixtures 12: 152 checks, plus documentation validation. |
| Fresh compilation | Two isolated static-runtime builds produced identical executable hashes; no dynamic CRT imports. |
| Static binary regressions | Context 23, session 23, everyday 28, native 33, package/installer 19, PE/PATH 4 and receipt-rejection cases 7 passed. These intentionally exercise the release-recipe binary, rather than adding new cases to the foundation count. |
| Recipient acceptance | All 16 PowerShell-only checks passed against the actual ZIP/installer, including restricted PATH, bare command lookup, nested discovery, project separation, boundary exit codes and recovery. |
| Final artifact inspection | All six external checksum entries and seven ZIP payload hashes/lengths matched; ZIP executable matched the fresh receipt; packaged quickstart matched current source. |
| Licensing | Apache-2.0 license retained; notices/inventory matched 49 locked Windows dependency packages. No legal certification claimed. |
| Documentation | Local links/anchors and filename casing, fences, PowerShell syntax, isolated README core commands and provider help-only examples passed. Mermaid node/edge syntax and references checked using the documented restricted grammar. |

Candidate: `.build/phase11-preview/swapctx-0.1.0-windows-x64.zip`.

```text
ZIP SHA-256     bd194942e6eb66fd3aa4f460ab5b0be6fb8934ebb3288609a5c6072892bdc7ba
Binary SHA-256  f37e809984ab00e3f5a33997e27b9bf3e9c098be8639934edaa1f0f7a78f405d
```

The native adapter is embedded through the unchanged Rust source and included
in the complete code inventory. Current build helper hashes match the new
receipt. The memory schema, Cargo dependencies, application/adapter code and
license/notice sources remain unchanged from Phase 10. Documentation is hashed
separately in [machine-readable evidence](evidence/phase11-release-preparation.json).
The owned compile tree was removed after the build.

No real provider launch, authenticated session, inference/API call, network-
dependent test, benchmark or soak run occurred. Offline evaluator fixtures do
not substitute for model evidence. The native suite's intentional input-timeout
diagnostic is expected and its suite passed.

### Release-path defect found during preparation

The first fresh producer run passed the foundation gate but stopped compiling
bundled SQLite: MSVC C1083 could not open its generated output beneath the nested
preview/build staging path. A targeted real C-compiler fixture confirmed the
cause: a 71-character output path succeeded and a 306-character path failed
with C1083. No model/provider was involved and no candidate destination appeared.

`scripts/build-windows.cjs` now puts independent compile trees in owned system
temporary storage while retaining destination-adjacent final assembly. It checks
ownership before cleanup, attempts both cleanups and preserves an original error.
The same fresh producer command passed after the fix, serving as the end-to-end
regression for this actual deep checkout. The first attempt's logs are retained
locally and its failure/probe are recorded separately in the evidence. No
compiler or integrity assertion was relaxed.

## Release gates

Severity below is release-decision priority, not a CVSS rating or an assertion
that a vulnerability has been found.

| Severity | Gate | Current boundary |
| --- | --- | --- |
| High | Current dependency advisory review | No locally available current advisory database established; online review outstanding. |
| High | Private vulnerability intake | GitHub private reporting enablement is unverified. Maintainer must choose and verify the channel before public release. |
| Medium | Hosted CI | Workflow source exists; no workflow dispatched or remote evidence fetched in this phase. |
| Medium | Independent clean-machine acceptance | Local restricted-PATH acceptance does not establish a clean Windows environment. No new physical-machine test required for this phase. |
| Medium | Publisher signing/provenance | Unsigned local receipts/checksums; signing or explicit unsigned-preview risk acceptance needed. |
| Scope limitation | Native compatibility | Synthetic contract evidence, not certified real Codex/Claude loading, trust, receipt or adherence. |
| Scope limitation | Reproducibility | Same-machine build comparison cannot establish cross-machine reproducibility. |
| Presentation limit | Hosted rendering | External URLs were not fetched. GitHub light/dark/mobile rendering and a full Mermaid renderer were not exercised; the design uses native Markdown and a constrained flowchart syntax. |

No network-dependent review, real-provider session, inference, model benchmark,
soak test, upload, GitHub settings change or publication is authorized by this
phase. Passing local gates prepares a release decision, not a public release.

Publisher options include a maintainer-managed Authenticode certificate or an
appropriate signing service. No provider/service selection, certificate purchase,
signing operation or authenticated attestation was performed. Same-machine build
receipts and repository commit IDs do not authenticate a distributed executable.

No current advisory database was found at the conventional local locations
checked (`%USERPROFILE%/.cargo/advisory-db`, `.build/cargo-home/advisory-db`,
`%LOCALAPPDATA%/rustsec/advisory-db`). No stale database was used as security
clearance. A fresh authorized online review remains outstanding.

The local acceptance report records Windows `10.0.26300.0` and PowerShell
`5.1.26100.9711`. Developer tools were removed from the test process PATH, not
uninstalled. Persistent user PATH registration and independent machine approval
are not demonstrated. A new physical-laptop test is not required to complete
Phase 11; the release decision must review available external evidence or
explicitly accept the limited preview scope.

## Approval handoff

The maintainer can review the local candidate and source changes now. Before
public distribution: complete current advisory review, verify private security
intake, review hosted/independent Windows evidence and decide signing or explicit
unsigned-preview risk acceptance. Keep native hooks labeled as offline-verified
adapters until separate live-provider validation is authorized and succeeds.

Any push, tag, upload, package submission, visibility change, hosted workflow
dispatch or public release requires explicit approval. Future live-provider
evaluation also needs separate authorization. No such actions occurred here.

## Files changed

| Files | Purpose |
| --- | --- |
| `README.md` | Native-workflow hero/demo, architecture, install/quick start, capability boundaries, compatibility, status and FAQ. |
| `CONTRIBUTING.md`, `SECURITY.md`, `SUPPORT.md`, `CHANGELOG.md` | Verified development commands, security intake decision, troubleshooting and unreleased change history. |
| `.github/ISSUE_TEMPLATE/bug_report.md`, `.github/ISSUE_TEMPLATE/feature_request.md`, `.github/pull_request_template.md` | Focused reports and review guidance using synthetic data. |
| `scripts/check-docs.cjs` | Local Markdown/anchor/fence checks, PowerShell AST parsing, restricted Mermaid validation and isolated README command checks. |
| `scripts/verify.ps1`, `.github/workflows/ci.yml` | Wire documentation checks into local/Windows gates; add the existing native offline suite to Windows CI. Workflow was not dispatched. |
| `scripts/build-windows.cjs` | Shorter independent compile staging, destination-adjacent assembly and ownership-checked cleanup preserving the first error. |
| `docs/USAGE.md`, `docs/RELEASE_PREPARATION.md`, `docs/WINDOWS_RELEASE.md` | Current CLI lifecycle/recovery reference, producer procedure and self-contained native-launch instructions in the packaged quickstart. |
| `docs/PHASE8_NATIVE_PROVIDERS.md`, `docs/WINDOWS_RELEASE_ENGINEERING.md` | Add pointers to current release status while preserving historical evidence. |
| This report and `docs/evidence/phase11-release-preparation.json` | Verification results, artifact identity and explicit release gates. |

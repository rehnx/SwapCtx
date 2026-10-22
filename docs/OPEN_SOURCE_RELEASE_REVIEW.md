# Open-source preview release review

Reviewed 2026-10-23 (Asia/Kolkata). **NO-GO: final GitHub and Windows gates
could not be completed in this execution environment. No release was published.**

## Source and PR review

Live Git refs confirmed main at
`95f9a632bed4b1a99977ceb5fb8bcdbd54705124` and PR #21 at
`015a28dae58952ffead62ecce80399ea2f09c022`. The public PR page reports OPEN.
Its diff changes SECURITY.md only and replaces contradictory pending-intake text
with GitHub private reporting instructions. It introduces no product code or
credentials. The owner confirms that private reporting is enabled and usable;
this session did not submit a report or independently inspect administration.
Required checks, reviews and mergeability could not be verified through the API.
PR #21 was not merged by this session; its intended changes were kept out of this
follow-up branch to avoid duplicating the PR.

The owner authorizes an unsigned experimental prerelease after the hard gates
pass, including separate hosted Windows acceptance. Older launch documents are
historical gate records; their pending authorization/intake language does not
supersede that current authorization. Older candidate hashes and owner PC tests
do not establish acceptance of any newer build.

## Prepared changes

- Add Cargo project/readme metadata while retaining version `0.1.0-preview`,
  Apache-2.0 and the existing lockfile.
- Retain candidates only through an explicit manual workflow option on `main`;
  preserve seven-day artifacts and run checksum-verified Windows 2022 recipient
  acceptance. No release-write permission or public-PR artifact publishing added.
- Clarify source availability, preview status, installation and experimental
  provider limits in README. Document the manual retention procedure.

## Verification in this session

The pinned Rust 1.99.0 toolchain and Node 24.19.0 were used on Linux.
Formatting and Clippy with warnings denied passed. All 114 Rust tests passed in each of debug and release profiles; one optional
performance test was ignored in each. The locked release build passed. Deterministic tests use temporary
stores and fake providers, with no authenticated provider or inference.

Node results: 23 context checks, 23 session tests, 28 everyday tests and 34 native
tests passed; 12 Windows-only native cases were skipped on Linux. Strict build
verification (3), supplemental SQL (10), pilot evaluator (12) and soak evaluator
(11) checks passed. These evaluator tests make no model requests. Documentation
validation passed with 91 local links; PowerShell parsing was skipped on Linux.
The modified workflow parsed as YAML, but its Windows jobs have not run here.
Windows packaging, static CRT, producer identity and recipient acceptance are
unverified for this branch. Final hosted dependency advisory results are unavailable.

The 57-package locked Windows dependency notice check passed after separately
provisioning the Windows dependency cache. Cargo.lock was not changed.
Gitleaks 8.24.3 matched the CI-pinned archive checksum and scanned 56 reachable
commits with no findings; a working-directory scan also found no leaks.
Secret scanning is heuristic, not a guarantee.

## Unavailable release capabilities

GitHub API calls for PR #21 and release ID `421160748` returned `Forbidden`.
`api.github.com` is absent from this environment's allowed HTTP hosts, and
`gh auth status` failed. No callable GitHub connector is available. Public HTML
and Git reads work. Git push authorization is evaluated separately; a dry-run
branch push succeeded. No security or network restriction was bypassed.

Consequently, this session could not verify protected-branch settings, hosted
checks, advisory results, issues #5/#6/#7/#10 or the existing draft assets; merge
PR #21; create/merge a follow-up PR; dispatch Windows Actions; obtain a final
Windows package; reconcile/publish the draft; or verify hosted downloads.
No tag, mainline merge, draft asset replacement or obsolete-branch deletion was
performed. The existing draft and its old candidate were left untouched.

## Resume conditions

Provide an authenticated GitHub capability with API access and repository/Actions
permissions. First re-read live state and merge PR #21 only after its applicable
checks pass. Merge and verify this follow-up before selecting the frozen main
SHA. Dispatch manual retention on that SHA, preserve producer and recipient
artifacts, and inspect actual hashes and all final-main security/advisory checks.
Only then update the existing `v0.1.0-preview` draft, verify downloads and publish
as a prerelease. Never reuse the old `e46b7a1` binary for the new source revision.

No final Windows ZIP, executable or installer hash is supplied here because none
was produced or verified in this environment. Linux binaries are test outputs,
not Windows release assets. Clean personal-machine testing and live native
provider compatibility remain unverified for the final candidate.

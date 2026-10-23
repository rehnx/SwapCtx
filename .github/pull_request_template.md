## Problem and resulting behavior

Describe the concrete trigger and what changes for the user.

## Verification

List exact offline commands and results. Identify skipped or unverified checks.
Use synthetic data and fake providers; no real-model verification is required.
For documentation-only changes, run `node scripts/check-docs.cjs PATH_TO_BINARY`
and check added pages/external links. No release rebuild or retag is needed.

## Compatibility and risks

Explain impacts on existing stores, CLI syntax, Windows packaging and privacy.

- [ ] Scope is focused; documentation matches the implementation.
- [ ] Existing data and opt-in sharing are preserved.
- [ ] Appropriate deterministic checks passed.
- [ ] No credentials, real project memory or transcripts are included.
- [ ] Claims distinguish local preparation/output from provider receipt/adherence.

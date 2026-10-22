# Public preview decision record

Prepared 2026-10-22. **Public-launch decision: NO-GO pending the gates below.**
This is engineering preparation, not permission to change visibility, publish a
release, create a public download or distribute a candidate externally.

## Delivered engineering

The unified candidate retains the native provider CLI and memory intelligence
branches, schema-3 storage/recovery, context continuity, intent, inspection,
explanations, verification and transactional handoffs. Historical branches are
preserved. The first follow-up fix is source
[`22c956e`](https://github.com/rehnx/SwapCtx/commit/22c956e3ab9264e8d9ab80e52bbb72e90151ff9e):
optional handoff IDs/counts, normalized intent matching, shared structural/field
validation, duplicate-member refusal and consistent memory-audit snapshots.

The current candidate adds bounded, repeatable handoff provenance, exact duplicate-memory diagnostics, a narrowly scoped
synthetic-value scanner configuration, a full-history CI scan, community conduct,
optional seven-day candidate retention in the private manual Windows workflow,
this decision record, [release-note draft](RELEASE_NOTES_0.1.0.md) and
[roadmap](ROADMAP.md). Final PR/CI outcomes are authoritative; a local pass does
not establish a hosted Windows pass.

## Evidence and limits

- First fix: formatting, Clippy with warnings denied and 104 Rust tests passed in
  each debug/release profile; one existing opt-in performance test ignored.
- Current memory/handoff candidate: 113 Rust tests passed in each profile with the
  same ignored performance test. Linux native fixtures explicitly skip 12 Windows
  cases. Windows packaging/PowerShell and fresh producer results must come from CI.
- Locked Windows third-party inventory: 57 packages matched. Dependency advisory
  clearance must be refreshed for the final candidate by the hosted advisory job.
- Gitleaks 8.24.3 scanned all reachable refs (32 commits at the first scan). The
  17 findings were deliberately synthetic redaction fixtures in four source/test
  files. Exact-value exceptions cover only three known fixture strings; no paths
  or commits are excluded. A repeated scan found zero remaining findings.
  The downloaded scanner matched its upstream release checksum. The workflow pins
  that digest. Detection remains heuristic and is not proof of secret absence.
- No installed provider, authenticated session, inference or paid service was used.
  Stores/exports are plaintext; provenance is self-reported; fingerprints unsigned.

## Outstanding gates

| Gate | Evidence still needed |
| --- | --- |
| [Private reporting and publisher trust](https://github.com/rehnx/SwapCtx/issues/5) | Confirm a usable private intake; decide signing or explicitly unsigned preview and trusted hash delivery. |
| [Independent Windows acceptance](https://github.com/rehnx/SwapCtx/issues/6) | Test the final exact candidate on an independent machine; retain source, receipts and all asset hashes. |
| [Native compatibility](https://github.com/rehnx/SwapCtx/issues/7) | Separate authorization and exact-provider evaluation, or retain explicit experimental/unverified support language. |
| [Windows reproducibility](https://github.com/rehnx/SwapCtx/issues/13) | Preserve raw differences; identify native compiler/linker/SDK and verify final same-toolchain builds. Cross-machine byte equality is not claimed. |
| Final source and assets | All hosted checks, a fresh static-CRT producer and recipient tests for the accepted revision; preserve that exact package. |
| Founder launch authorization | Approve visibility, publication and external distribution only after reviewing the completed evidence. |

Execution credentials could read code, push branches, manage issues and start CI.
Reading branch-protection and Actions-permission settings returned HTTP 403;
private-vulnerability-reporting inspection returned 404, which does not establish
its state. Repository description/topics updates also returned 403. No repository
security, permission or visibility settings were changed. Do not infer verified
private intake from these responses.

## Final release procedure

1. Select the reviewed, passing mainline revision and record the commit and CI URL.
2. Produce a fresh candidate using [release preparation](RELEASE_PREPARATION.md).
   Record exact executable, ZIP and installer SHA-256 from its own evidence.
3. Complete independent Windows recipient acceptance and the private-intake/trust
   decisions. Keep native support claims limited to evidence actually obtained.
4. Replace pending identity fields in the release-note draft. Proposed version and
   tag are `0.1.0-preview` / `v0.1.0-preview`; no tag is created by this document.
5. Present source, assets, hashes and limits for founder approval. Only then change
   visibility, publish the release or distribute assets externally as approved.

## Founder handoff

Follow the [owner launch checklist](OWNER_LAUNCH_CHECKLIST.md) for private security intake, exact independent Windows acceptance, and the approval boundary. Native Codex/Claude compatibility remains experimental; live verification is not part of this preview gate.

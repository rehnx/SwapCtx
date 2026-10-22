# Unified integration candidate

Engineering review: 2026-10-22. This is a development candidate, not release
approval or native-provider certification. Work was performed in an isolated
Linux checkout; the maintainer's Windows worktrees and uncommitted files were
not accessed or modified.

## History and merge decisions

- Remote `main`: `b84b3cd08a3f175de7c731a9b81d650b4a284871`.
- Task branch `codex/unified-reliability` started at `feat/unified-integration`
  `6181a7b`, including native hook stabilization `8bb571d`.
- Merged `feat/intelligence-reliability` `7fc80fd`, including `dc7aac2` and
  `d323452`, as merge commit `a96f08a`. There were no textual conflicts.
- Both input branches already descended from `b84b3cd`; no separate mainline
  merge or cherry-pick was needed. Phase 9–11 remains in the ancestry.
- No history was rewritten. Main was not changed. Candidate head is the PR head;
  final evidence records the tested source inventory independently of report-only
  commits.

## Architecture and retained behavior

The Rust dispatcher combines core memory/lifecycle/recovery, manual policies,
native provider selection, the older managed Codex console and all intelligence
commands. SQLite remains schema 3; migrations, provenance, expiry, conflict
handling and project-scoped context retrieval retain the original architecture.
There is no new schema, daemon, storage network client or automatic ingestion.

Native Codex/Claude interfaces remain provider-owned. Plain launch shares no
memory; context mode is explicit, capability-gated, scoped, fresh and validated
per lifecycle event. Stabilized Codex hook definitions and launch-local metadata
from the UX branch remain intact. Local callback preparation/output never proves
provider receipt or model adherence. The historical managed-console pilot does
not certify this candidate's native integrations.

## Defects fixed after merging

| Problem | Result and regression evidence |
| --- | --- |
| Git porcelain paths are repository-relative, while nested projects matched/read them as project-relative. | Shared read-only Git evidence scopes status/diffs/inventory to the project and normalizes its repository prefix. A nested-project test checks deny rules and sibling-independent fingerprints. |
| Boundary inspection matched only lexical names and could follow external links while scanning. | Match lexical and resolved internal names; refuse outside scope for policy checks and skip inaccessible external content. Synthetic Unix symlink tests cover both boundaries. Existing Windows path safeguards are reused. |
| Safe worktree edits could hide credentials staged in the index. | Scan bounded index blobs and current files; strict verification rejects skipped changed content. A staged-secret regression proves later safe edits cannot produce a passing verdict. |
| Lossy Git decoding could substitute another filename; malformed rename records were accepted. | Parse NUL records fallibly, preserve Unicode/newlines, refuse invalid Unicode and malformed evidence. Cross-project renames fail closed. |
| Inspection diff previews could invoke configured external diff/textconv programs. | Disable those helpers and optional index writes; use literal pathspecs. Git executable/configuration and same-account filesystem access remain trust inputs. |
| JSON path explanations exposed secret-like requirement/rule fields despite text redaction. | Apply the same secret-display filtering to both formats, validate project paths and propagate invalid packet requests. |
| Some new commands queried absent legacy tables/columns. | `intent list` and audit support recognized old stores without writes. Commands needing provenance give explicit schema-3 guidance. Additive write migration preserves legacy records. |
| Handoff inputs were unbounded and ambiguous shapes/counts could be silently accepted. | Require object/arrays, reject duplicate keys and inconsistent declared counts, cap inputs at 16 MiB/10,000 entries per array, retain transactional conflict rollback. |
| Export wrote directly to the destination; imported `intent` could escape mandatory requirement counting. | Sync and publish a temporary file without overwrite; normalize imported intent to requirement. Test explicit mandatory coverage and conflicting intent refusal. |
| Fingerprinting accumulated project file bytes in one vector. | Stream file content in 64 KiB chunks into SHA-256 while retaining length-delimited fields and the existing versioned fingerprint contract. |
| Native suite silently returned success for Windows-only cases on Linux. | Mark 12 cases skipped explicitly; repair a hard-coded `.exe` fixture and run portable native/docs fixtures on Ubuntu CI too. |
| Added SHA-256 dependencies were absent from the Windows notice inventory. | Regenerate notices from locked, checksum-verified crate archives: 57 Windows packages. No additional application dependency was added during integration. |

The daily handoff workflow also gains explicit `--include-decisions` and
`--include-notes`. Defaults remain requirements/intent only; all exported kinds
retain active/valid/fresh/unconflicted/known-origin and secret-heuristic filters.
These flags neither send data nor start a provider.

## Verification and artifacts

Environment: Linux x86_64, Rust 1.99.0, Node 24.19.0. The merged baseline passed
formatting, Clippy and 83 Rust tests before integration fixes (one performance
test ignored). Current candidate validation uses:

```text
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked --release
cargo test --locked --release --all-targets
node scripts/check-sql.cjs
node scripts/check-context.cjs target/release/swapctx
node scripts/check-session.cjs target/release/swapctx
node scripts/check-everyday.cjs target/release/swapctx
node scripts/check-native.cjs target/release/swapctx
node scripts/check-docs.cjs target/release/swapctx
node scripts/check-pilot-harness.cjs
node scripts/check-soak-harness.cjs
node scripts/third-party.cjs --check
cargo audit --json
```

Exact final local counts, source/log hashes and advisory-database identity are
recorded in [machine-readable evidence](evidence/unified-integration.json).
Logs are retained in the task workspace at `/workspace/swapctx-evidence`, outside
the repository. They contain only deterministic fixtures and tool output.
Offline pilot/soak harnesses test evaluators, without running models or soaks.

The CI matrix retains debug/release Rust, portable Node fixtures, Windows native
and packaging checks, and advisory review. A new dependent Windows job provisions
dependencies, then runs `prepare-preview.ps1` into a new disposable directory:
the deterministic offline gate, fresh static-CRT comparison, packaging and
recipient acceptance. It prints evidence but uploads no distribution. CI outcomes
must be taken from the linked run, not inferred from workflow configuration.

## Release boundaries and continuation

This Linux environment cannot execute the authoritative Windows PowerShell gate,
MSVC/static-CRT comparison, Windows shell/junction tests, PE checks or recipient
acceptance locally. Review hosted Windows results for the exact PR revision.
No distributable from an older phase is recertified by this source merge.

Stores, backups, handoffs and provenance are plaintext/self-reported. Secret
heuristics can miss credentials. Inspection/verification are snapshots with
limited file scanning, not an atomic filesystem authorization mechanism. An
unsigned SHA-256 fingerprint is not a signature, independent review or provider
receipt. Audit facts and filesystem evidence can change between reads; re-run
before a consequential review decision. Preview only displays a limited sample
of a bundle; larger transfers require full-file review.

No installed provider, authenticated session or real inference was run. No
release/tag, visibility/settings change, mainline merge, certificate purchase,
distribution upload or deployment occurred. Local advisory review is dated
evidence, not security certification. Private vulnerability intake, independent
clean-machine acceptance, signing/authenticated provenance and exact native
compatibility remain maintainer release decisions.

Recommended next development:

1. Review this PR's Windows CI and fresh producer evidence; fix any platform
   defects before accepting the candidate. Keep it draft while material evidence
   is missing.
2. Review command contracts, mandatory imported intent, skip reporting, conflict
   rollback and exact dependency notices. Use the [daily guide](INTELLIGENCE_WORKFLOWS.md)
   examples with disposable stores.
3. With separate authorization, evaluate exact installed Codex/Claude versions
   under normal trust policy. Test actual hook loading, startup/resume/clear/fork
   and compaction events, then assess provider receipt and adherence separately.
4. Before public distribution, establish private security intake, obtain or
   explicitly scope clean-machine evidence, decide signing/unsigned-preview trust,
   refresh dependency advisories and run the fresh producer for the final source.

Changed-file scope is available in the PR diff: merged native UI/hooks and fixtures;
intelligence/audit modules and tests; integration hardening in context, paths,
storage and Git evidence; CI, dependency notices and current workflow documentation.

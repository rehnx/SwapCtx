# Daily memory and agent handoff workflows

SwapCtx preserves deliberately saved knowledge. It does not collect conversations,
expand model context windows, or establish what an agent knows. Commands below
use the selected registered project and the same global `--data-dir` as your
ordinary commands. Memory, exports and backups are plaintext.

## Start with project intent

```powershell
swapctx init
swapctx intent add 'Preserve public interfaces' --key compatibility
swapctx memory add --kind decision 'Use the existing SQLite store'
swapctx memory add --kind note 'Next investigate startup latency'
swapctx intent list
swapctx memory audit
swapctx memory audit --json
```

Intent is stored as a non-expiring `requirement`, so existing mandatory-context
coverage applies. `intent pin ID` clears expiry on an active, valid requirement;
it refuses notes, decisions and inactive records. `intent retire ID --reason TEXT`
invalidates a requirement while retaining history. Labels and origins are
self-reported; they do not authenticate an architect or make a statement true.

The read-only audit reports readiness, conflicts, expiry, provenance and omissions.
Inventory and simulated context coverage share one read transaction and expiry
timestamp. Sensitive optional records are included in review recommendations;
missing origin rows produce a blocked diagnostic without repairing the store.
Exit 0 means the diagnostic ran; read its readiness field rather than treating
exit success as healthy memory. `READY` describes local memory prerequisites,
not provider installation, hook loading or receipt. Optional stale notes can be
excluded while mandatory context remains complete. Resolve competing knowledge
deliberately with review, supersession, invalidation or archival. Exact duplicate
candidates share kind (ignoring ASCII case), content and key among active valid
records. Audit reports only their decimal-string IDs in `quality.duplicates`, with group/record
counts and review guidance. It does not merge them or compare semantic similarity;
review their provenance before archiving redundant records. Different keys, kinds,
projects and inactive records are kept distinct.

## Understand the selected context

```powershell
swapctx explain --id 1 --json
swapctx explain --query 'interfaces architecture' --max-bytes 7168
swapctx explain --path src/auth/login.rs --json
```

IDs come from your own store. Record explanations report local inclusion or an
omission reason without printing record content. Query mode emits the existing
`swapctx.context` v1 packet when JSON is requested. Its budget accepts 2,048–131,072
UTF-8 JSON bytes, not tokens. Path explanations show relevant requirements and
matching boundary rules; secret-like values are hidden in both output formats.
Path inputs are relative to the registered project root. Symlink aliases and
resolved destinations are checked; outside destinations are refused.

The historical JSON field `delivered` means inclusion in a locally generated
packet only. Provider receipt and model adherence remain unverified. Invalid
packet requests or generation failures return nonzero instead of being labeled
as ordinary budget omissions.

## Review the worktree

```powershell
swapctx inspect --json
swapctx inspect --path src/auth --diff
swapctx verify --strict --attest
swapctx verify --strict --json
```

These commands require Git. They scope evidence to the registered project,
including when it is a subdirectory of a Git repository. Rename sources and
destinations are checked. Cross-project renames and unsupported path identities
fail closed. Paths with spaces and NUL-delimited Git names are preserved;
non-Unicode filenames are refused rather than changed into another identity.

Inspection returns 1 for deny matches or detected credential markers, 0 otherwise.
Ask matches are review warnings; no approval is requested or granted. JSON includes
per-file `sensitive_status`: `clean`, `detected`, `oversized_skipped` or
`unreadable_skipped`. Files over 256 KiB, binary files and inaccessible paths
are not certified clean. Both staged contents and current files are scanned;
later safe edits cannot hide staged credentials. The diff preview is a redacted,
limited view of the unstaged diff, not a complete staged review.

Verification returns 1 when a check fails. Strict mode also rejects ask matches,
expired active memories, review-required context and unscanned changed files.
Boundary policies remain manual observations, without filesystem enforcement.
Git status avoids optional index writes; diff inspection disables external diff
and text conversion helpers. No configured remote or provider is contacted.

The `attest:swapctx:v2:sha256` value binds selected project memory/lifecycle fields,
rules, Git HEAD, staged and unstaged diffs, the file inventory and file contents
to the check results. File contents are hashed in bounded chunks. It is an
unsigned state fingerprint, not a signature, proof of authorization, independent
review or provider receipt. It is project/machine dependent, reads the project
inventory, and does not provide an atomic filesystem snapshot or protection
against same-account races. Re-run after changes.

## Switch agents with a reviewable bundle

```powershell
# Default: eligible requirements and intent only. Choose a new output file.
swapctx handoff export handoff.json --target claude --include-rules
# Opt in to additional knowledge; still fresh, unconflicted and secret-filtered.
swapctx handoff export handoff-with-notes.json --include-decisions --include-notes
swapctx handoff preview handoff.json
# Run from the receiving registered project with the same intended data store.
swapctx handoff apply handoff.json
```

Export does not send a message or start an agent. It includes active, valid,
fresh and unconflicted entries with known self-reported provenance, filtered by
secret heuristics. Decisions and notes require the corresponding explicit flag.
This is a curated transfer, not a whole-project backup; use SQLite export/backup
for complete history. Review the plaintext bundle before sharing. Heuristic
filtering cannot guarantee absence of sensitive values.

JSON handoff v1 accepts at most 16 MiB and 10,000 entries per array. Both arrays
are required; duplicate JSON members, inconsistent declared counts and malformed
field types are rejected in both preview and apply. IDs and declared counts are
optional; preview counts the actual arrays. Both paths normalize kinds without
regard to ASCII case and treat intent as a requirement. Internal competing keys
and rule definitions are rejected before target-store matching. Export syncs a temporary file and publishes without overwriting;
the parent directory must exist. Apply validates fields and commits all records,
origins, history and optional rules in one transaction. Competing keys and rule
collisions refuse the entire apply; exact structural matches deduplicate.
Imported `intent` kinds become mandatory `requirement` records. Original source
and actor labels are preserved verbatim; the receiving target is recorded in the
local import event. Repeated transfers do not grow source labels. Missing sources
receive a bounded handoff label, and explicit unknown provenance remains unknown
for default context eligibility. Existing labels and history remain unchanged.
Preview shows the first five entries/rules and conflict counts; inspect the full JSON separately
when approving a larger transfer. It does not apply or approve the bundle.

## Compatibility and safe upgrades

Core reads and `intent list` continue to support recognized legacy stores without
migration. `memory audit` reports old schemas as blocked. Context-dependent
workflows, handoff, inspection and verification require schema 3 and explain how
to migrate rather than issuing missing-table errors. Back up first; an explicit
write command such as `init` migrates transactionally. `doctor` remains read-only.

Machine contracts remain `swapctx.inspect` v1, `swapctx.verify` v1,
`swapctx.memory_audit` v1 and `swapctx.handoff` v1. CLI parser errors return 2;
operational failures return 1. Existing core, native UI and managed Codex commands
remain available. Native sharing still requires explicit `--context` and a fresh
validated packet for each supported lifecycle callback.

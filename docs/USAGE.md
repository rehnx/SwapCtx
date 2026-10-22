# CLI usage and recovery

Install first using the [Windows guide](WINDOWS_RELEASE.md). Examples assume
`swapctx` resolves to the intended executable and run from a registered project.
Use synthetic data when experimenting.

## Projects and storage

`swapctx init` registers exactly the current directory. Project commands select
the nearest registered ancestor. Separately registered nested projects have
separate records/rules; parent rules are not inherited. Default data lives in
`%LOCALAPPDATA%\SwapCtx`, independently of the installation directory.

Use a global `--data-dir 'C:\Private\SwapCtx'` on **every command** to choose a
different store. Prefer absolute paths when changing directories. Ordinary reads
do not create or migrate a store. Recognized legacy stores remain readable;
writes can migrate them transactionally. Back up before upgrading, and keep the
older executable until the new installation is verified. Downgrades are not guaranteed.

## Memory lifecycle

```powershell
swapctx memory add --kind requirement --key authentication --source 'reviewed specification' 'Preserve authentication behavior'
swapctx memory add --kind decision --expires-in-days 30 'Reuse the current storage layer'
swapctx memory search 'authentication'
swapctx memory recent --limit 10
swapctx memory get 1
swapctx memory history 1
swapctx memory supersede 1 'Preserve authentication and session expiry behavior'
swapctx memory review 2 --expires-in-days 60
swapctx memory recent --include-inactive
```

The IDs above assume a fresh store; use the IDs printed by your own commands.
Kinds are user labels; `requirement` receives mandatory-context treatment.
Search is literal substring matching, not semantic/vector search. Explicit keys
identify competing active statements; conflicts are reported, not silently merged.
Origins identify self-reported source/actor labels and are not authenticated.

`memory archive ID` retains a record while excluding it from default retrieval.
`memory invalidate ID --reason TEXT` preserves invalid knowledge and its reason.
`supersede` atomically links a replacement while retaining the original.
`review` records explicit review; expiry changes are optional. There is no automatic
truth detector or conversation ingestion. Inspect potentially sensitive text only
with an intentional `memory get ID --reveal`; storage remains plaintext.

## Context and native providers

```powershell
swapctx context --protocol-version 1 --query 'authentication architecture' --max-bytes 8192
swapctx native codex
swapctx native claude
swapctx native codex --context
swapctx native claude --context
```

Only the `--context` forms opt into lifecycle sharing. Both keep the original
provider UI. The interactive menu asks separately; the default answer is No.
Legacy `swapctx launch codex`/`claude` perform plain native handoff only.
Install/authenticate providers separately. Native executable discovery excludes
the current directory. Windows `native` accepts native `.exe` files or validated
standard npm package entries; it never interprets the shell shim. JS entries need
Node. Use `--executable ABSOLUTE_PATH` and `--node ABSOLUTE_PATH` if needed; context
and health checks require Node 24+. See the [native setup guide](PHASE8_NATIVE_PROVIDERS.md)
for supported installation layouts.

`context` reports coverage and exclusions in versioned JSON. Read-only selection
omits stale/conflicting/invalid/secret-like/unknown-origin records by default;
missing mandatory coverage must be checked, not inferred from exit success alone.
Native validation refuses incomplete mandatory context. Read the
[protocol](CONTEXT_PROTOCOL.md) and [native contract](PHASE8_NATIVE_PROVIDERS.md)
for exact budgets, event sources, argument restrictions and version gates.

The optional older managed console is preserved under `swapctx codex`;
[its setup and observatory guide](PHASE5_EVERYDAY_CODEX.md) documents that separate
workflow. It is not required for native provider use. Model/provider commands
must not be used as routine offline verification.

## Manual boundary policies

```powershell
swapctx protect add --path 'src/auth/**' --reason 'Changes need review' --policy ask
swapctx protect check 'src/auth/login.ts'
swapctx protect list
```

Patterns use project-relative forward slashes; check targets resolve from the
calling directory. All matches are reported. Deny takes precedence over ask,
then audit. No policy intercepts an operation, requests approval or grants permission.

| Exit | Meaning |
| --- | --- |
| 0 | Success, no match, or audit-only match |
| 1 | Operational, validation or database error |
| 2 | Deny match **or** CLI parser usage error; inspect diagnostics |
| 3 | Ask match without deny; no approval was requested or granted |

Checks examine supplied and resolved names, including in-project junctions, and
reject traversal, outside destinations and ambiguous Windows paths. Windows
pattern matching is ASCII insensitive, not full Unicode filename equivalence.
Checks do not enumerate descendants, cover every hard-link alias or prevent a
race between checking and writing. Check each affected path.

## Backup, transfer and restore

Create `C:\Backups` first, or choose another existing private directory. All
outputs below must be new. Snapshots and bundles are plaintext, including history.

```powershell
swapctx backup 'C:\Backups\swapctx-snapshot.db'
swapctx export 'C:\Backups\project-context.db'
# From the destination registered project: preview first, then apply deliberately.
swapctx import 'C:\Backups\project-context.db'
swapctx import 'C:\Backups\project-context.db' --apply
# The restore directory must not exist; its parent must exist.
swapctx --data-dir 'C:\Backups\NEW-recovery-data' restore 'C:\Backups\swapctx-snapshot.db'
swapctx --data-dir 'C:\Backups\NEW-recovery-data' doctor
```

Backup uses SQLite's online snapshot API for the whole store, including committed
WAL changes and excluding uncommitted writes, then verifies and publishes without
overwriting. Restore validates before creating a new explicit directory. It never
replaces a working store. Interruption can leave the new directory or staging;
inspect it and choose another new directory for retry.

Export includes only the selected project's records, rules, provenance and
history in a fresh schema-3 SQLite bundle. Import previews without writes;
`--apply` merges transactionally by stable origin IDs and remaps local links.
Identical copies deduplicate; divergent origins or cross-project origin reuse
refuse the merge. No SQL dump is executed or conflict silently resolved.
Transfer bounds are 256 MiB input, 100,000 rows and 64 MiB decoded values per table.
Larger stores can use whole-store snapshots. Memory backups do not include
provider-owned history or managed adapter settings/session references.

## Diagnostics and failure recovery

```powershell
swapctx status
swapctx doctor
swapctx privacy
swapctx capabilities
```

Doctor checks database integrity, foreign keys and lifecycle relations without
repair. Unknown, corrupt, foreign or future schemas are refused. Preserve the
entire data directory, including SQLite WAL/SHM/journal sidecars. Never delete
sidecars or reset records as a repair strategy. `init` is not corruption recovery.
For safe report collection, see [support](../SUPPORT.md).

## Memory intelligence and review

The unified CLI also provides `intent`, `inspect`, `explain`, `handoff`, `verify`
and `memory audit`. See [daily workflows and machine contracts](INTELLIGENCE_WORKFLOWS.md)
for examples, exit codes, explicit decision/note transfer, scan coverage and schema
upgrade guidance. All evidence remains local; hashes are unsigned fingerprints.

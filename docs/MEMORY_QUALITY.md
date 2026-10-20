# SwapCtx Memory Quality & Health Diagnostics

## Overview

SwapCtx manages project memory locally and offline using SQLite. Over long coding sessions, project knowledge evolves: requirements change, temporary notes expire, decisions are superseded, and competing statements may be recorded under shared keys.

If project knowledge becomes inconsistent, expired, or conflicted, **native context delivery to coding agents is withheld** to prevent models from hallucinating or operating on invalid instructions.

The `swapctx memory audit` command provides a read-only, non-mutating health inspection of the currently registered project's memory. It evaluates storage integrity, quality lifecycle metrics, and native context readiness, providing actionable guidance to resolve competing, stale, or incomplete records.

---

## Command Reference

### `swapctx memory audit`

Assesses the current project's memory and prints a structured diagnostic assessment.

```bash
swapctx memory audit
```

#### JSON Output Mode

For automation, CI pipelines, and scripting:

```bash
swapctx memory audit --json
```

Emits a machine-readable JSON packet with schema `swapctx.memory_audit v1`.

---

## Evaluated Health Diagnostics

The audit evaluates nine specific dimensions of project memory health:

### 1. Total and Active Memory Records
- **Total records:** All memory records saved in the project across its lifetime.
- **Active records:** Records with `state = 'active'` and `validity = 'valid'`. Only active and valid records participate in default search, recent listings, and native context retrieval.

### 2. Active Valid Requirements
- Requirements (`kind = 'requirement' COLLATE NOCASE`) represent mandatory project constraints.
- Native context retrieval (`swapctx context` and `swapctx native <provider> --context`) requires **100% mandatory coverage** of all active valid requirements. If even a single requirement is excluded (due to expiration, key conflict, unknown provenance, secret patterns, or size budgets), native context delivery fails with `mandatory_context_incomplete`.

### 3. Expired and Outdated Memories
- Memories created with `--expires-in-days <N>` carry an explicit `expires_at` Unix epoch timestamp.
- When `expires_at <= current_time`, the record is marked `STALE` and excluded from fresh-only retrieval.
- **Critical distinction:** An expired general note simply drops out of fresh search, but an **expired active requirement breaks native context preparation**.
- **Remediation:** Renew with `swapctx memory review <id> --expires-in-days <N>`, make permanent with `--clear-expiry`, or archive with `swapctx memory archive <id>`.

### 4. Conflicting Active Records (Key Conflicts)
- Memories may specify an explicit `--key <KEY>` to group competing statements (e.g. `--key auth`).
- When two or more active, valid records share the same key in a project, a **key conflict** exists.
- In native context retrieval, conflicted keys are excluded by default (`--include-conflicts = false`). If any of the competing records is a requirement, mandatory context is incomplete.
- **Remediation:** Use `swapctx memory supersede <old_id> <new_text>` to create an official successor, or `swapctx memory invalidate <id> --reason <reason>` to mark obsolete statements invalid.

### 5. Invalidated, Archived, and Superseded Records
SwapCtx never deletes records. Instead, it maintains persistent lifecycle history:
- **Invalidated:** Explicitly declared untrue or invalid (`swapctx memory invalidate <id> --reason "..."`). Retains the invalidation reason and origin in history.
- **Archived:** Preserved in historical queries but excluded from default active memory (`swapctx memory archive <id>`).
- **Superseded:** Atomically linked to a replacement record (`swapctx memory supersede <id> ...`), preserving lineage without cycles.

### 6. Unknown or Insufficient Provenance
- Schema version 3 introduced structured origin tracking (`memory_origins`), recording `origin_id`, `source`, `actor`, and registration timestamps.
- Records imported from legacy schemas (v1/v2) carry self-reported labels `source = 'legacy-unknown'` and `actor = 'unknown'`.
- Native context retrieval excludes unknown provenance by default (`--allow-unknown-provenance = false`).
- **Remediation:** Active requirements with unknown provenance should be superseded or re-added with explicit `--source <label>` and `--actor <label>`.

### 7. Memories Excluded from Native Context Retrieval
The audit simulates the exact native context packet generation used by provider SessionStart hooks (`PACKET_BYTES = 7168`, `limit = 100`, unconflicted, fresh only, verified provenance only) and enumerates which active records are excluded and why:
- Stale / Expired
- Competing key conflict
- Unknown or legacy provenance
- Potential sensitive / secret patterns detected by safety guards
- Packet byte budget overflow

### 8. Mandatory-Requirement and Boundary-Rule Coverage
- Compares active valid requirements against requirements delivered in the packet.
- Compares stored boundary rules (`swapctx protect`) against rules packed in the packet.
- Flags whether any review-required records are present (stale, conflicted, or unverified).
- Confirms whether `mandatory_context_complete` evaluates to `true`.

### 9. Native Context Readiness States
The audit synthesizes all diagnostics into an overall readiness status:

| Readiness | Meaning | Native Hook Behavior |
| :--- | :--- | :--- |
| **`READY`** | All active requirements and boundary rules are clean, fresh, unconflicted, and fit in the packet budget. | Context is delivered to coding sessions cleanly. |
| **`NEEDS ATTENTION`** | Project storage is structurally valid, but active memories contain conflicts, expirations, unverified provenance, or secret markers that cause context to be withheld or degraded. | Native hooks fail with `mandatory_context_incomplete`. Context is withheld. |
| **`BLOCKED`** | A fundamental store prerequisite fails: database schema < 3 (migration required), foreign key / integrity corruption, invalid rule matcher patterns, or packet envelope budget failure. | Native hooks abort before session initialization. |

---

## Actionable Guidance

The audit does not merely output raw counts; it suggests exact, prioritized actions to restore memory health:

1. **Conflicting Keys:** Identifies conflicting keys and specific record IDs (e.g. `key "auth" (records #2, #5)`), suggesting `swapctx memory supersede` or `swapctx memory invalidate`.
2. **Expired Requirements:** Identifies the requirement ID and provides commands to review, renew, clear expiry, or archive.
3. **Unknown Provenance:** Identifies requirements lacking provenance and explains how to supersede them with explicit sources and actors.
4. **Sensitive Markers:** Identifies records flagged by secret-detection heuristics so they can be replaced without sensitive tokens.
5. **Schema Upgrades:** Identifies outdated schemas and directs the user to run `swapctx doctor` or a write command to upgrade to schema v3.

---

## Privacy and Safety Guarantees

1. **Strictly Read-Only:** `swapctx memory audit` opens the SQLite database in `SQLITE_OPEN_READ_ONLY` mode. It never writes to the database, alters rowids, or updates timestamps.
2. **No Automatic Mutations:** Does not automatically expire, supersede, merge, or delete records. Human review remains in control.
3. **Sensitive Text Protection:** Potentially sensitive record text is **never displayed** in audit summaries or exclusions. Record IDs, kinds, keys, and sanitised metadata are shown, but content remains hidden unless explicitly requested via `swapctx memory get <id> --reveal`.
4. **Completely Local & Offline:** No network requests, telemetry, background daemons, transcript logging, or external API calls.

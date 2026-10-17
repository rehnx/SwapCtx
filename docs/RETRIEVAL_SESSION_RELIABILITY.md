# Retrieval and session reliability

Maintenance follow-up, Windows 2026-10-17. Rust 1.99.0, Node 24.17.0. Existing Phase 5 work was preserved. No dependency, storage migration, context protocol version or native agent capability was added. All implementation verification stayed offline; no Codex invocation, external AI agent, provider request, soak test or soak self-test ran.

## Engineering choices

Review covered the Rust CLI, storage/schema validation, memory/history, path checks, protection rules, context serialization, backups/transfers, optional adapters, diagnostics/evaluation entry points, tests and development evidence. The existing transactional storage/recovery design was retained. Three issues had more immediate everyday value than another integration feature:

1. **Useful memory disappeared under byte pressure.** The selector kept only the highest-ranked records and then popped records from the end until the complete packet fit. A large, high-ranked optional note could cause all smaller notes to be removed before it was finally removed itself. With a record limit of one, an oversized newest record could leave an empty selection even when an older record fit. A regression against the previous executable reproduced one returned requirement instead of the four fitting records expected.
2. **Unchanged knowledge could be reinjected into conversation history.** The adapter fingerprint included omission/scan counters and budget metadata. Archiving excluded history or adding withheld data could therefore change the hash and trigger an acknowledged full-snapshot injection, although selected records and mandatory coverage were unchanged. This adds requests/data to the integration without a corresponding knowledge update.
3. **Damaged local references were validated too late.** Session metadata was checked after starting Codex and reading configuration/account metadata. Several presence checks also treated dangling Windows junctions/symlinks as absent. These should fail locally and preserve the suspect reference, including when the user requests a new thread.

## Implementation

Context retrieval now retains at most 5,000 lightweight candidate descriptors, sorts them once with the existing requirement/term/recency ranking, and encodes candidates as needed for selection. It tries whole ranked records against the complete serialized packet budget, skips non-fitting records, and continues to smaller candidates. Rules retain priority. The record limit applies to records that fit. ASCII case variants and repeated query terms count once instead of multiplying relevance weight.

Both passes remain inside the same read-only SQLite transaction. Full integrity, provenance and lifecycle checks, conflict exclusion, secret heuristics, mandatory-coverage refusal and capped-scan refusal are retained. No record is shortened or rewritten. Counter-width reservation can leave a few bytes unused; selection is greedy, not mathematically optimal. Whole-store checking/conflict grouping still scale with database size. The 5,000-candidate cap is not a bound on all database work. No new throughput or peak-resource benchmark claim is made.

The adapter's **selection fingerprint v2** includes the protocol/schema, canonical project identity, selection policy, mandatory coverage, scan-truncation state, ordered selected records with provenance/freshness, and boundary rules. It excludes retrieval time, budget/omission/scan counts, and per-record `matched_terms`/`selection_reason` explanations. Actual counts/explanations remain in the ordinary context packet and diagnostics. The hash identifies selected knowledge and safety policy; it is not a hash of every wire byte or evidence of native receipt/model behavior. A ranking change that changes record order can still change the hash.

`contextHashVersion: 2` is additive saved-reference metadata; `hashVersion: 2` is additive selection/status/observatory metadata. The existing `swapctx.context` and `swapctx.codex` protocols remain v1, and saved-reference `version` remains 1. Existing references are accepted, including historical references without a policy hash. The first successful managed operation with an old fingerprint refreshes it through the existing acknowledged injection path, preserves the thread ID, and saves v2; unchanged later operations need no additional history injection. Native runtime developer instructions continue to receive the current packet before each operation. Conditional-RPC behavior is tested with in-process fixtures; this milestone does not establish actual token/billing savings.

Session-reference parsing is shared by status and the managed engine. It checks shape, project identity, native-home path, thread identifier and hash fields before version checking/native startup. Invalid JSON, foreign/future references and dangling references fail as `invalid_session_state`; no replacement or repair occurs, even with `--new`. Existing old homes need not still exist when deliberately starting a new thread; this does not grant access to another home or migrate its history. Remaining settings, lock, metadata-directory and status presence checks recognize dangling links rather than treating them as absence. Filesystem checks remain cooperative, not a defense against every hostile same-account race.

## Offline verification

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -DeterministicOnly -ReleaseTests
```

Formatting, Clippy with warnings denied, release build and **47 native tests in both debug and release** passed. Supplemental suites passed: **10 SQL, 23 context/legacy adapter, 23 transport, 27 everyday/session/observatory/evidence, and 12 pilot-oracle fixture checks**. The optional performance benchmark remained ignored. [Verification record](evidence/retrieval-session-offline-2026-10-17.json).

The new context cases cover the reproduced budget failure, fallback under a one-record limit, distinct-term ranking and seven budgets with Unicode escapes, mandatory requirements, rules and changing omission-counter widths. The first expanded Unicode fixture incorrectly assumed all saved notes fit an 8 KiB packet. That assertion was corrected to query the escaped record specifically; it did not justify increasing the budget or weakening byte checks. The final complete verification run passed.

Adapter fixtures verify that excluded/secret-like history and relevance explanations do not trigger an unnecessary injection; selected knowledge and expiry changes still do. Other tests preserve developer-policy updates, exercise legacy one-time refresh, and reject invalid JSON/null/future/foreign/dangling references before any version check or native startup. Tests use disposable projects and the real compiled Rust CLI; lifecycle protocol operations use in-process fake APIs. Original user stores, native history, settings and earlier evidence files were not modified. Earlier uncommitted Phase 5 changes remain present.

## Everyday operation and remaining limits

Continue using the existing commands from a registered project:

```powershell
& $SwapCtx codex status
& $SwapCtx codex run --observe
# A second terminal in the same project/store:
& $SwapCtx codex watch
```

See [the everyday guide](PHASE5_EVERYDAY_CODEX.md) for the executable variable, setup, Node override, custom storage and memory lifecycle commands. No reset/reinitialization or user-data migration is needed. `contextCurrent` may be false for an old saved fingerprint until the next successful managed operation refreshes it. A rejected damaged reference must be preserved and inspected; `--new` does not silently discard it.

The improvements establish deterministic local selection, metadata behavior and constructed lifecycle requests. New adapter changes have not been exercised against a native Codex process or a real model, as required by this task's verification constraint. Existing authenticated pilot evidence remains historical. Natural automatic recovery, general instruction adherence, comparative benefit, total agent/tool overhead, other Codex versions, hosted Windows/Linux CI and online advisory auditing remain unverified. Storage/backups are plaintext, provenance remains self-reported, and secret detection is heuristic. Observation does not enforce file protection or inspect hidden model state; external memory does not expand native context.

The next useful work is release packaging and user-driven reliability fixes on the pinned setup. More integration surfaces or a rewritten storage engine would not close these remaining evidence gaps.

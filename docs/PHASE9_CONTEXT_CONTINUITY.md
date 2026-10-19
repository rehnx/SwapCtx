# Phase 9: native context continuity and recovery

Work began 2026-10-18; final review completed 2026-10-19. Starting local HEAD:
`680f1c4`; working tree was clean.
Implementation and verification remained local, offline and provider-free.

## Audit and decisions

Phase 8/8.5 already retrieves a new project-scoped context packet for every
configured SessionStart source: **startup, resume, compact, clear and fork**.
`src/context.rs` obtains facts, coverage counts and origins in one read-only
SQLite transaction and diagnoses lifecycle consistency before returning data.
The native adapter independently validates protocol/schema, canonical scope,
nested-project boundaries, mandatory coverage, freshness/expiry, conflicts,
provenance, sensitive-value heuristics, UTF-8, duplicate keys and byte budgets.
Those paths are preserved. No storage engine, schema migration, transcript
ingestion, additional lifecycle event or provider integration was introduced.

The missing continuity capabilities were phase-level observations, a stable
identity for equivalent validated snapshots, and recovery when diagnostic
storage fills. Previously the receipt cap could disable all subsequent context
callbacks. The receipt also could not distinguish a callback interrupted before
preparation from one that had never run, or preparation from completed writing.

## Refresh and deduplication semantics

Every permitted callback retrieves and validates a **current** snapshot, even
when an earlier snapshot had the same identity. Failed snapshots withhold context;
there is no old-text fallback, negative cache, automatic retry, forced compaction
or provider restart. Launch preflight remains a separate validation, not a cache
for the later callback. The callback's event directory still determines retrieval;
a registered child project cannot borrow its parent's context.

Fingerprints use domain-separated HMAC-SHA-256 over canonical JSON **after full
validation**. Object keys are sorted; array order is retained. The registered
project identity is canonicalized. Only `generated_unix` and transport
`budget.bytes_used` are excluded. Requirements, rules, origins, expiry, coverage,
omission/conflict counters and additive fields remain in the identity. Clock-only
or object-key-order changes do not change it; semantic changes do. Invalid,
expired or conflicted mandatory context cannot acquire a usable fingerprint.

A random 32-byte key is generated once per native launch and held in its temporary
adapter configuration. Thus identity is stable **within that launch**, not across
independent launches. This avoids an unkeyed persistent content hash that could
be correlated between sessions. This key is diagnostic metadata, not provider
authentication, and fingerprints are not tamper evidence or proof of truth.
Legacy temporary configs without the additive key still work without fingerprint
comparison. No public context protocol field changes.

Deduplication applies only to the set of diagnostic snapshot identities read at
native exit. Repeated preparations are counted, with an explicit “not suppressed”
label. No context text is deduplicated or cached. The existing contracts provide
neither a SwapCtx receipt acknowledgement nor a trustworthy callback replay ID
that would justify suppressing identical context. Resume, compact, clear, fork
and even repeated startup events therefore remain eligible for full delivery.
There is no claimed reduction in model tokens or provider requests.

## Observations and bounds

The disposable receipt file records these stages:

| Stage | Meaning |
| --- | --- |
| `callback_requested` | Adapter entry was invoked; input has not necessarily been parsed yet |
| `retrieval_attempted` | Event/source and directory scope passed; a fresh local read is about to run |
| `context_prepared` | Packet validated; optional keyed fingerprint recorded |
| `output_written` | The local stdout write callback completed successfully |
| `context_withheld` | Input, scope, retrieval or validation failed, with a safe error code |
| `output_failed` | Output writing failed; partial receipt may be possible |

A random local callback identifier relates these stages. It is not a provider
session identifier. The initial source is `invalid` until input is parsed; later
records use only the five allowed source names. The summary reports prepared
contexts, completed writes, failure observations, unfinished callbacks and
repeated snapshots. Failure observations can outnumber callbacks (for example,
invalid context followed by an advisory-write failure).

**Provider receipt and model adherence are always unknown.** An unfinished
observation means no terminal stage was recorded, not proof of a specific crash.
The receipt format is internal v2 metadata, not a new public API. Malformed or
truncated receipts yield a warning, never echoed private contents or a delivery
claim. The old Phase 8 evidence remains historical.

The log is capped at 262,144 bytes, including the next appended line. A short
exclusive append lock prevents concurrent cooperating callbacks from overrunning
that cap. There is no lock wait or retry. Capacity, contention, an abandoned lock
or write failure disables observations for that callback, emits a safe warning,
and best-effort creates an empty `.incomplete` marker. It **does not disable
validation or withhold an otherwise valid snapshot**. Exit summaries with such
a marker/lock explicitly say counts are incomplete. Metadata is not a dependency
for context recovery and is not guaranteed durable or tamper-proof.

Existing stdin (32 KiB/3 seconds), retrieval (5 seconds), packet (7,168 bytes),
final context (8,192 bytes) and configured provider hook (10 seconds) bounds remain.
No background polling, per-message interception or new network client exists.
Fingerprinting operates on the bounded packet; the diagnostic set is bounded by
the receipt cap and constructed only at exit. No performance improvement is claimed.

## Privacy and native behavior

Continuity logs contain stages, safe codes, allowlisted source names, random local
IDs and keyed fingerprints. They contain no memory text, project paths, prompts,
provider session IDs, transcript paths, conversations or credentials. The temporary
configuration still needs local executable/storage/project paths and now contains
the ephemeral fingerprint key. Local filesystem permissions apply; this is not
encryption or protection against another process running as the same user.

The original native UIs, opt-in sharing, authentication, trust and policy remain
provider-owned. No instructions or provider settings are edited. These commands
retain their existing meaning:

```powershell
swapctx
swapctx launch codex
swapctx launch claude
swapctx native codex
swapctx native claude
swapctx native codex --context
swapctx native claude --context -- --continue
```

The managed `swapctx codex` commands and memory/context/backup/protection/maintenance
commands are unchanged. Ordinary launching remains the compatibility fallback.
Unsupported probes preserve native arguments; supported context launches retain
their scope/configuration argument restrictions. Exit codes and existing owned
temporary-directory cleanup are preserved. Forced termination can leave temporary
metadata and scripts; no crash-proof cleanup or process-tree supervision is claimed.

## Offline verification and remaining gates

The existing 27 native cases were retained, with relevant phase assertions updated.
Five new cases cover canonical/semantic fingerprints; identical callbacks without
suppression; stale failure followed by recovery and cross-project forks; full or
contended diagnostic storage; and killing a real Node callback before input EOF,
then successfully invoking another callback. Existing timeout/invalid-retrieval
cases now assert that preparation is never reported for those failures.

Commands run using installed Rust 1.99.0, cached Cargo dependencies and Node 24.17.0:

```powershell
cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo build --locked --offline
cargo test --locked --offline --all-targets
node scripts/check-native.cjs target/debug/swapctx.exe
```

Formatting, Clippy and build passed. Rust: **54 passed, one existing performance
test ignored**. Native: **32 passed**. The final diagnostic-boundary fixture also
checks one byte below capacity, ensuring an append cannot exceed the cap.
The expected timeout rejection emits a safe stderr diagnostic. See
[machine-readable evidence](evidence/phase9-continuity-offline-2026-10-18.json).
No package rebuild, provider/model launch, authenticated session, inference call,
benchmark, soak or network-dependent test was run.

After the full suite, a final guard ensures that any error after context output
starts cannot emit an advisory falsely claiming no memory was supplied. The
broken-stdout, fake-native-UI and diagnostic-capacity cases passed again, and the
development binary was rebuilt. These targeted reruns are not additional cases
in the 32-case total. The requested evidence filename retains its October 18 date;
its final-review date and refreshed hashes identify the completed local work.

Provider contracts and admission gates remain those documented in
[Phase 8](PHASE8_NATIVE_PROVIDERS.md) and
[Phase 8.5](PHASE8_5_NATIVE_VERIFICATION.md). No exact native provider release is
certified by this work. Actual config loading, hook trust, lifecycle scheduling,
automatic compaction handling, provider receipt and model adherence still require
separately authorized live-provider validation. Offline callbacks demonstrate the
adapter's response to an event, not that every provider will emit that event.
Clean-machine/hosted verification and distribution recertification were not run.

Changed files: `integrations/native/native.cjs` (fingerprints and bounded stage
diagnostics), `scripts/check-native.cjs` (continuity regressions), the Phase 8 guide
(changed diagnostic behavior), this report and the Phase 9 evidence file. No Rust
source, SQLite schema, Cargo dependency, CLI interface or provider configuration
contract changed. All changes are local; nothing was committed or published.

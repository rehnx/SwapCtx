# Security policy

SwapCtx is preparing its first Windows preview. There is no promised security
support period, certified provider release range or security certification.
Current review evidence is in [Phase 10](docs/PHASE10_PRODUCTION_HARDENING.md) and
[the release assessment](docs/PHASE11_RELEASE_READINESS.md).

## Reporting a vulnerability

Private vulnerability reporting is enabled for the public SwapCtx repository.
Verified that GitHub displays the private **Report a vulnerability** form, including advisory details, impact, affected products and severity fields.
The reporting form was not submitted. The security policy is being updated to document the confirmed reporting route.
Unsigned-preview publisher trust remains a separate release decision.

If the repository's **Security** tab offers **Report a vulnerability**, use that
private GitHub form. Otherwise, request a private contact method from the
maintainer without posting exploit details, credentials, private paths or data.
Do not put sensitive vulnerability reports in a public issue or pull request.
Before public release, the maintainer must establish and verify a private intake
route and update this policy with the confirmed instructions.

A report should identify the affected source revision or binary hash, Windows
version, impact and a minimal reproduction using synthetic data. Include
mitigations if known. Do not attach a real memory database, provider transcript,
authentication configuration or environment dump.

## Security boundaries

- Memory databases, SQLite bundles and backups are **plaintext**. Permissions
  inherit from the local account and destination directory. No encryption,
  dedicated ACL isolation or tamper-proof audit history is provided.
- Sensitive-value detection is heuristic. It can miss secrets and flag harmless
  text. Explicit sensitive storage remains plaintext; inspect exports/history
  before sharing. Provenance labels are self-reported, not authenticated facts.
- Retrieval remains local. Opting into a provider integration exposes selected
  context to that provider and potentially its configured services and history.
  Providers own their authentication, permissions and network behavior.
- Native adapters validate bounded snapshots and scoped lifecycle events. A
  successful local write does not prove provider receipt or model adherence.
  A provider can skip a hook because of version, trust, policy or configuration.
- Boundary rules perform manual snapshot checks. They neither intercept writes
  nor grant authorization. Filesystem races, hard-link aliases and malicious
  same-account processes are outside any claimed enforcement boundary.
- Trusted executable paths, non-workspace PATH entries, `SystemRoot`, local
  toolchains and account permissions remain trust inputs. SwapCtx is not a
  sandbox against an attacker who controls those inputs.
- Unknown/corrupt databases are refused rather than reset. Preserve the whole
  data directory, including SQLite sidecars, and restore a valid backup into a
  new directory. Never delete WAL/journal files as a repair attempt.
- ZIP/payload hashes detect changed bytes when compared with a trusted expected
  hash. They do not authenticate the publisher. Current candidates are unsigned;
  build receipts are local evidence, not authenticated provenance.

SwapCtx introduces no telemetry, automatic transcript collection, background
service or storage/retrieval network client. Optional provider processes retain
their own behavior. No current online dependency-advisory clearance is claimed.

## Safe investigation

Use temporary projects, synthetic memories, fake executables and deterministic
offline tests. Preserve original stores and backups. Do not probe an authenticated
provider or submit model requests without separate authorization. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the offline gate.

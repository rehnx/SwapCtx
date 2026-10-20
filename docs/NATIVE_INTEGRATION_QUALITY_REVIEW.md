# Native integration reliability and product quality

Date: 2026-10-20. Baseline HEAD: `4bf04f0`. The source-only publication review's
uncommitted documentation changes were present at the start and are preserved.
This is an engineering sprint, not a new phase or release preparation.

## Follow-up: package-declared native Windows executables

Baseline for this targeted fix: `87b679d`. A reported Claude Code 2.1.304 npm
installation declares `bin/claude.exe`; the resolver incorrectly admitted only
JS/CJS/MJS entries. It now also accepts a declared, canonical, contained `.exe`
and routes it through the existing direct executable target. The shell shim is
never interpreted. Plain launching requires no Node lookup; context/check still
requires Node.js 24+. Health reports `ready (native executable)` without probing
the executable. No Node adapter change is needed: executable targets already
support capability probes, native launching and lifecycle configuration.

Exact package identity, duplicate-key rejection, relative path validation,
canonical package/prefix containment and current-executable recursion rejection
remain required. Unsupported extensions and escaped paths remain rejected.
This does not authenticate the package publisher or certify Claude 2.1.304 hooks.

`tests/fixtures/native-provider-exe.rs` builds a disposable synthetic executable
which invokes only the existing local JS fixture using explicit absolute paths.
`scripts/check-native.cjs` tests the declared `bin/claude.exe` layout: read-only
health with no provider probe, plain launch without Node on PATH, literal
arguments, exit status, no database creation, and the actual CLI-to-adapter-to-hook
chain for startup/resume/compact/clear/fork with refreshed requirements and cleanup.
Negative fixtures cover traversal, junction escape, alternate streams, missing
entries and self-recursion. Existing JS package regressions remain in place.
The fixture uses the local Rust compiler; it never starts an installed provider.

Changed files for this follow-up: `src/native.rs`, `scripts/check-native.cjs`,
`tests/fixtures/native-provider-exe.rs`, this report, `README.md`, `SUPPORT.md`,
`docs/PHASE8_NATIVE_PROVIDERS.md` and `docs/WINDOWS_RELEASE.md`. Existing
uncommitted publication documentation and other untracked files are preserved.

Verification for this follow-up (Windows, offline): targeted executable fixture
passed, then `scripts/verify.ps1 -DeterministicOnly` exited 0. Formatting, Clippy
with warnings denied, 55 Rust tests (one performance test intentionally ignored),
release build and all 159 supplemental deterministic checks passed, including
40 native checks and 19 packaging checks. Documentation verification passed
(12 files, 59 local links; provider examples checked through help only).
Logs: `.build/npm-exe-verify.stdout.log` and `.build/npm-exe-verify.stderr.log`.
No installed provider was launched, no real npm installation was modified, and
no model/API call or publication occurred. Real-provider compatibility remains
unverified; the reported 2.1.304 installation was reproduced synthetically.

## Findings and fixes

| Confirmed gap | Result |
| --- | --- |
| The menu/`native` route rejected every Windows npm shell shim, although legacy `launch` could start one. | Standard `@openai/codex` and `@anthropic-ai/claude-code` npm installs can resolve their declared JS entry point and invoke Node directly. The shim's contents are never parsed or executed. Plain launching and opt-in context use the same resolved command. |
| Missing executable and unsupported installation layout were conflated. | `unsupported_provider_layout` identifies missing/ambiguous/unsafe package metadata or unsupported shim layouts and suggests a native executable or plain legacy launching. Missing executables have separate actionable diagnostics. |
| Native launching lacked the legacy launcher's self-recursion rejection. | Canonical paths resolving to SwapCtx itself are refused with `provider_self_recursion`. |
| No equivalent local native health command existed. | `native codex --check` / `native claude --check` separately report provider resolution, Node runtime, store/project scope, context validation and local hook generation. They never start or probe a provider. |
| Rust temporary-directory cleanup could replace a nonzero adapter/provider exit status. | Cleanup warns while preserving the primary nonzero status. A cleanup failure after success still returns an error. No process-tree/crash-proof cleanup guarantee is added. |
| Prior positive tests stopped short of Rust CLI → embedded adapter → compatible synthetic provider for both providers. | New npm-layout fixtures exercise that complete chain, including actual generated hook invocation and output observation. This demonstrates the local pipeline, not a real provider's parser or receipt. |

During implementation, the full chain also caught Node rejecting a Rust canonical
verbatim `\\?\` script path. Paths remain canonical for containment checks, then
use conventional drive/UNC spelling for Node's main-module resolver. The fixture
compares project directory identity using filesystem metadata so Windows alias
spellings do not produce a false mismatch. These failures were resolved before
the final gate.

## Installation and security boundaries

| Route | Windows layouts | Context sharing |
| --- | --- | --- |
| `swapctx launch codex` / `claude` | Existing legacy `.exe`, `.cmd`, `.bat` handoff | None; unchanged. |
| `swapctx native codex` / `claude`, or menu answer No | Native `.exe`, or recognized standard npm prefix | None. Native executable needs no Node; npm JS entries need Node, while package-declared native `.exe` entries launch directly. |
| `swapctx native codex --context` / `claude --context`, or menu answer y | Same resolution; Node.js 24+ required for the adapter | Explicit opt-in; existing lifecycle admission and validation apply. |
| `swapctx native codex --check` / `claude --check` | Same file resolution; Node.js 24+ checks the adapter | No provider process, no supplied context to a provider. |

Native executable discovery is preferred. If no native executable is found,
absolute PATH prefixes excluding the current directory and its canonical aliases
can supply standard npm `.cmd`/`.bat` markers. Explicit overrides must be absolute.
Npm resolution reads at most 64 KiB of UTF-8 `package.json` using the already
bundled SQLite JSON parser in memory. It requires the expected package name,
unambiguous JSON keys, a declared provider bin, a contained JS/CJS/MJS or native EXE file and
canonical package containment within the installation prefix. Traversal, drive/
ADS forms, control characters, unsupported entry-point extensions and junction escapes fail closed.

No new dependency, SQLite schema change, disk parser database or shell evaluation
was introduced. Argument values remain individual process arguments, including
spaces, quotes and shell metacharacters. Arbitrary batch/PowerShell shims and
pnpm/Yarn/package-link indirection outside the prefix are not interpreted. Use an
underlying native `.exe` or the existing trusted plain legacy launcher instead.
Package names and containment are not publisher authentication; installed
executables/packages and trusted PATH directories remain local trust inputs.
Node comes from the safe native resolver or `--node`; custom shim-selected
runtime flags and environment setup are not replicated.

Local inspection found a native Codex path and Node, but access to conventional
user npm package manifests was denied in this environment; no Claude installation
was found by passive command lookup. No actual provider was executed or modified.
The new layouts therefore have synthetic layout evidence, not certification of
the user's real npm/provider distribution. A wrapper can still lack an optional
native payload or require a different runtime; health does not execute it.

## Read-only health workflow

From the project (or a nested directory), using the same data directory as normal:

```powershell
swapctx native codex --check
swapctx native claude --check
# Optional explicit installation/runtime selection:
swapctx native codex --check --executable 'C:\Tools\Codex\codex.exe' --node 'C:\Program Files\nodejs\node.exe'
```

Exit 0 means local context prerequisites and configuration preparation succeeded.
Exit 1 means something is missing/unsupported/invalid; it does not imply ordinary
plain native launching is impossible. `--check` conflicts with `--context` and
forwarded provider arguments. It reports:

- File resolution and layout, without claiming provider loading succeeded.
- Node 24+ runtime gate. Missing Node gives an install/override instruction.
- Readable or missing/invalid store and registered-project scope separately.
- A fresh packet passing existing protocol, schema, canonical scope, freshness,
  conflicts, provenance, sensitive-data, duplicate-key, mandatory-coverage,
  UTF-8 and byte-budget checks. Completeness is relative to stored requirements,
  not proof that every real project requirement has been saved.
- Temporary hook configuration generated and cleaned locally. Provider loading,
  trust, exact compatibility, receipt and model adherence remain unknown.

No provider `--version`/`--help` probes run in health mode. No store is initialized,
migrated or repaired. No context text is printed or persisted; configuration/code
exist only in owned temporary directories. Missing or malformed memory never
falls back to an older snapshot. A context launch can still fail its later fresh
read even after a successful health check.

At actual context launch, the existing bounded provider capability probes remain
part of opt-in startup. Unsupported probes warn and open the original UI with
unchanged user arguments and without context retrieval. Supported but incomplete
mandatory context stops before the UI. If no callback occurs, the warning points
to `--check` for local prerequisites and provider trust/policy for the unverified
portion. Provider errors and nonzero exits remain visible; no restart is added.

## Offline verification

The synthetic fixture is `tests/fixtures/native-provider.cjs`. It is copied into
standard-shaped temporary npm installations whose shell shims are deliberately
unusable. It never imports a provider SDK, authenticates, contacts a service or
requests a model. Both providers exercise:

- Rust CLI, embedded adapter, version/help admission, generated Codex Windows
  command or Claude exec-form plugin, and all startup/resume/compact/clear/fork
  callbacks. An updated requirement after startup must appear in subsequent
  outputs. The test checks packet protocol, mandatory coverage and project identity.
- Plain launch and unsupported-probe fallback with literal special-character
  arguments; no unexpected store creation or shell-shim execution.
- Missing callbacks, malformed callback input, outside/nested-project mismatch,
  provider exit 29 and abrupt synthetic provider termination. Owned adapter/plugin
  directories are absent after these handled exits; diagnostics do not claim receipt.
- Health never even probes the synthetic provider. Store bytes remain unchanged;
  missing stores remain absent. Conflicting mandatory memory fails health and
  context preflight before the synthetic UI starts.
- Wrong/duplicate package names, traversal, ADS-shaped entry paths and junction
  escapes; missing Node, self-recursion and a synthetic pre-24 runtime gate.

The existing suite retains stale/sensitive records, malformed packets, bounded
events, hook timeout, broken stdout, interrupted callbacks, receipt limits and
cleanup-failure regressions. It still retrieves a new snapshot for every recovery
callback and never suppresses context because an identical one was prepared.

Final verification used Rust 1.99.0, Node 24.17.0 and cached dependencies:

```powershell
cargo fmt --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo test --locked --offline --all-targets
cargo build --locked --offline
node scripts/check-native.cjs target/debug/swapctx.exe
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File scripts/verify.ps1 -DeterministicOnly
```

The final deterministic gate exited **0**. Formatting, Clippy, debug tests and
release build passed: **55 Rust tests**, one existing performance test ignored.
Supplemental checks passed: schema 10, context 23, session 23, everyday 28,
native **39**, package/installer 19, PE/PATH 4 and offline oracle fixtures 12
(**158 total**), plus documentation checks. The six new native case groups cover
both providers and multiple failure scenarios; scenarios are not added again to
the case count. The complete 39-case native suite passed for both the debug and
final release executables. The final full-chain test forwards Codex's `resume`
form and Claude's `-c` form through the synthetic command, with literal special
characters retained; this does not validate real-provider argument grammar.

The native suite's intentional `hook_input_timeout` stderr message is an asserted
negative case, not a failed check. Package checks generated only disposable test
fixtures; no portable release candidate was built or refreshed. No release-profile
unit-test repetition, benchmark, soak or network-dependent verification was run.
A final targeted forwarding rerun initially used Node's `--test` mode with the
binary as a positional argument; Node treated the `.exe` as a second JavaScript
test file and reported a syntax error. This was an invocation error, not a CLI
failure. The corrected command passed against the release executable:

```powershell
node --test-name-pattern="actual Rust CLI" scripts/check-native.cjs target/release/swapctx.exe
```

Local gate logs are `.build/native-quality-verify.stdout.log` and
`.build/native-quality-verify.stderr.log`, with SHA-256:

```text
stdout 77a53fdaeb2c20003d8764bfba32b0362dfd57a9cdb900f0bab832c23604cf02
stderr 0244e6b05b0b076c16e8d97532359dd0080176d3a4bbc556ba9a711192039883
```

## Changed files

- `src/native.rs`: bounded npm metadata resolution, shell-free entry points,
  self-recursion rejection, `--check` and cleanup/exit handling.
- `src/main.rs`: capability reporting for local native health.
- `integrations/native/native.cjs`: script prefixes for probes/launch/fallback,
  provider-free health mode and actionable missing-callback diagnostics.
- `scripts/check-native.cjs`, `tests/fixtures/native-provider.cjs`: six focused
  scenario groups covering the full synthetic chain and installation/health safety.
- `README.md`, `SUPPORT.md`, `docs/WINDOWS_RELEASE.md`: targeted current usage
  updates preserving the pre-existing source-only publication edits.
- `docs/PHASE8_NATIVE_PROVIDERS.md` and this report: supported layout boundaries,
  health instructions, offline evidence and remaining real-provider gates.

The pre-existing changes to `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`,
`docs/RELEASE_PREPARATION.md` and `docs/FINAL_PREPUBLICATION_REVIEW.md` were left
untouched. Earlier evidence records and existing local release candidates were
not replaced or recertified.

## Remaining live-provider verification

No provider interface or admission version range was invented or broadened. Codex
still requires recognizable version/help advertising the existing hook/config and
trust capabilities; no minimum certified release is known. Claude still admits
major 2 from 2.1.296 with `--plugin-dir`; that is an admission floor, not an exact
certified range. The official contract references retained in
[Phase 8.5](PHASE8_5_NATIVE_VERIFICATION.md) and the
[native guide](PHASE8_NATIVE_PROVIDERS.md) remain the basis.

To establish genuine native compatibility later, with separate explicit approval:

1. Record exact provider, Windows, Node and installation layout; establish local
   file/runtime readiness first, including an npm wrapper's real native payload.
2. In disposable authenticated projects, verify the actual provider loads the
   generated config/plugin under its normal trust and administrative policy.
   Never bypass trust or change permissions to manufacture a pass.
3. Exercise actual startup/resume/clear/fork and manual/natural compaction where
   supported, recording which events the provider really emits. The synthetic
   fixture is not a certification of either provider's TOML/plugin loader.
4. Separate adapter preparation/output from independently observed provider/model
   receipt. Use controlled requirements and artifact checks to evaluate adherence
   separately; neither delivery nor adherence becomes a universal guarantee.
5. Verify real terminal Ctrl+C/process-tree behavior and cleanup for the exact
   Windows installation. Synthetic provider termination and mocked signal
   forwarding are narrower evidence; forced parent termination can leave temp files.

As before, exit statuses in the supported 0–255 range are preserved; signals and
out-of-range Windows exit statuses normalize to 1. No broader Windows exit-code
or process-tree guarantee is implied by the synthetic tests.

None of those live steps was run in this sprint. No release/distribution was
refreshed, no settings or user memory changed, and nothing was committed or pushed.

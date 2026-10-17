# Phase 7B Windows package-test failure

The failed manual preview run was inspected through read-only GitHub APIs:
[run 39061820171](https://github.com/rehnx/SwapCtx/actions/runs/39061820171),
job 117481170268, source commit
`640d296670e3ea1c3083f321a80460a24b7fbf8a`, Windows Server 2025 image
`windows-2025-vs2026`. No workflow was dispatched, and no remote state changed.

## Origin and cascade

The first failing package test was `portable package contains only the allowlisted
payload and correct hashes`. Its PowerShell error was **Refusing cleanup outside
staging parent**, at the packager's final cleanup guard. The absent ZIP reported
by later tests was a consequence: the shared fixture never finished packaging.

The guard compared `GetDirectoryName(GetFullPath(stage))` with a parent retained
from a different path-resolution operation. These are textual comparisons, not
directory identity checks. The hosted log contains both `C:\Users\RUNNER~1\...`
and `C:\Users\runneradmin\...` spellings of the runner's temporary tree. Windows
and .NET path normalization can preserve or expand an 8.3 alias depending on the
operation and host configuration. The same mismatch existed in the pre-publication
payload guard. A valid staging directory could therefore be refused.

The `finally` guard also replaced an earlier in-body exception. The old run does
not retain that exception's message, so it would be inaccurate to claim its exact
wording is known. The stale-notice negative test likewise reported the cleanup
guard instead of the intended integrity rejection. The observed guard failure,
the inconsistent-name comparison in the source, and the mixed-alias regression
establish the defect; the post-fix hosted outcome remains to be verified.

The remaining tests were independent top-level tests but used files created by
the first test. They continued after its failure and reported ENOENT, failed
launches and missing extracted directories. This obscured the first useful error.

## Fix

- Resolve each **existing parent directory** using `CreateFile` and
  `GetFinalPathNameByHandle` before constructing new destinations or staging paths.
  All comparisons then use the same normalized namespace, including short-name
  and junction aliases. Apply the same fix to packaging, installation, preview
  preparation and recipient acceptance so the next stage does not repeat the bug.
- Retain direct-parent containment checks. Reject reparse-point staging/work
  directories before recursive cleanup, including redirected payload directories.
  This is not a sandbox against a hostile process changing the filesystem between
  checks. The inline helper keeps installer/acceptance files self-contained and
  uses stock Windows PowerShell/.NET facilities, not Node, Rust or MSVC on the
  recipient. Non-DOS/UNC volume namespaces are refused explicitly.
- Preserve the primary package exception. If cleanup also fails, emit an
  additional diagnostic without replacing the original error. Cleanup failures
  on otherwise successful operations still fail the operation.
- Establish packaging/extraction as prerequisites before registering dependent
  tests. Failed setup now produces one failing test with the originating error;
  dependent ZIP reads do not run. Existing manifest, digest, rejection and
  overwrite assertions remain in place.
- Include process status, signal, launch/timeout errors, command arguments,
  stdout and stderr in test failures. A failed process launch is not accepted as
  a successful negative validation test.
- Create the acceptance fixture's `.build` parent explicitly on fresh checkouts;
  it is not assumed to exist on the hosted runner.

An initial directory-enumeration normalization attempt encountered an ancestor
listing restriction in this managed environment. It was replaced with handle
resolution, which does not enumerate the user's parent folders. No permissions
were broadened. The original full failure does not reproduce under this laptop's
normal path-normalization mode; the regression explicitly exercises differing
real 8.3/long spellings and confirms they identify the same directory.

## Offline verification

```powershell
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass `
  -File scripts/verify.ps1 -DeterministicOnly -ReleaseTests
node scripts/check-build-receipt.cjs .build/phase6-release/swapctx.exe .build/phase6-release/build-info.json
```

Format, Clippy, release build and 47 native tests in each debug/release profile
passed. All 17 package tests passed, including unchanged existing integrity and
installer tests plus new cases for:

- real short/long alias equality, packaging and installation through short names,
  alternate-spelling overwrite refusal and staging cleanup;
- initial packaging failure with one originating diagnostic and no cascading ZIP
  errors;
- fault-injected parent mismatch and staging junction, preserving the primary
  failure and an outside sentinel while refusing unsafe cleanup.

The package tests also ran the existing 16-check PowerShell-only acceptance
script with developer tools absent from its process PATH. Receipt checks cover
the static build's packaging path separately. Exact results are retained in
[offline evidence](evidence/phase7b-windows-ci-offline-2026-10-18.json).

No Rust/product behavior, database schema, saved data, context protocol, ZIP
manifest format, hash checks or signatures were changed. No model, agent or soak
evaluation ran. Existing distribution artifacts were not replaced or published.

## Still required

After user approval to push, rerun the Windows-hosted workflow on the patched
commit. Confirm the first package prerequisite, all package regressions, and the
subsequent preview gates pass on the runner's actual .NET/PowerShell configuration.
Only the old hosted failure was read here; new verification was offline and local.
Other public-release approval gates from Phase 6 remain unchanged.

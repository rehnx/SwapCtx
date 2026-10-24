# Windows terminal compatibility review

This records the implementation and local checks collected before committing on 2026-10-23,
based on `b08383e6deefbe3859cdb389ca9238eee2320b8d`. It does not certify a
published binary or establish Windows runtime compatibility from Linux results.
See the [installation guide](WINDOWS_RELEASE.md) for user commands.

## Audit and implementation

| Area | Finding and resulting behavior |
| --- | --- |
| `scripts/install-windows.ps1` | Already snapshots and verifies the archive, enforces bounded allowlisted extraction, checks each payload hash/length, resolves directory aliases and refuses existing destinations. These checks and cleanup guards remain intact. Success output now gives CMD and PowerShell discovery and session PATH instructions, with metacharacter guards. |
| `scripts/install-windows.cmd` | New thin entry point invokes the adjacent PowerShell script through the absolute Windows OS host, with profiles/prompting disabled. No download, extraction, verification or PATH logic is duplicated. Delayed expansion is disabled and no second `call` expansion is introduced. It returns the installation engine's exit code. Both files must be independently verified before execution. |
| `src/display.rs` | Previously treated terminal stdout as proof of ANSI and Unicode support. Now probes the actual Windows console mode, enables processed/VT output when supported, and selects the complete styled UI only with interactive handles and a known Unicode host. Unknown legacy hosts use ASCII, including escaped Unicode menu paths. Code pages and fonts are never changed. |
| `src/provider.rs` | The unstyled title contained an em dash. The fallback is now entirely ASCII, uses shared interactivity detection, and explains CMD/PowerShell use when redirection prevents selection. |
| `src/main.rs` | Clap help/errors follow the same capability decision, including empty `NO_COLOR`, so forced-color configuration cannot leak ANSI into redirected output. |
| `src/native.rs` and native adapters | Existing native launches already inherit console handles and resolve native EXEs or validated standard npm entries independently of the invoking shell. No shell replacement was needed. Native resolution, opt-in sharing, provider hooks and their PowerShell interpreter remain unchanged. |
| Release tools and tests | Packaging adds a checksummed CMD sidecar without expanding the ZIP payload allowlist. Package expectations and independent recipient checksum counts are updated. CI, `verify.ps1`, and the fresh producer run the new shell regressions. Static CRT, binary equality, receipt and backup checks remain intact. |

There are no changes to memory handling, SQLite schemas, storage/migration code,
provider adapter sources, backup/restore behavior, Cargo dependencies or lockfile.
Current published preview assets have not been replaced or relabeled.

## Regression coverage

[`check-windows-shells.cjs`](../scripts/check-windows-shells.cjs) tests CMD and
Windows PowerShell 5.1, plus PowerShell 7 when installed. An installed but broken
PowerShell 7 fails instead of being silently skipped. It covers:

- Fresh and repeated installations, preserved existing files, spaces and Unicode.
- Incorrect archive hashes and altered payloads with a matching outer ZIP hash.
- Adjacent installer lookup, missing companion errors, literal argument forwarding
  and exact exit-code propagation through the CMD wrapper. A fake PowerShell on
  PATH must not intercept the OS host selection.
- Session PATH discovery, memory storage/retrieval, redirected selection refusal,
  and native fake-provider exit propagation without changing saved memory.
- Interactive selection through real console handles in each shell, using a
  private Win32 console. The fixture waits for the prompt, feeds q/Enter or selects
  a fake provider without context, and checks the screen and exit status. Cases include unknown legacy
  hosts, nonempty/empty `NO_COLOR`, `TERM=dumb` and a simulated Windows Terminal
  marker. VT starts disabled so the styled case exercises actual enablement.

Portable tests cover every capability combination, control escaping and Unicode
path escaping, plus redirected help, errors, status and menu refusal with forced
color configured. Existing native/provider, data recovery and security tests
remain in place. All provider fixtures are local fakes; no live provider ran.

## Local verification

The execution environment was Linux with Rust 1.99.0 and Node 24.19.0.

| Check | Result |
| --- | --- |
| `cargo fmt --check`; Clippy with `--locked --all-targets -- -D warnings` | Passed |
| Debug and release Cargo suites with `--locked --all-targets` | Each passed 117 tests; one existing performance benchmark ignored |
| `cargo build --locked --release` | Passed, Linux binary |
| Node build verification, SQL, context, session, everyday, native, pilot-oracle and soak-oracle checks | 144 passed; 12 native checks skipped because they require Windows |
| Documentation links, examples and notice inventory | Passed; notices cover 57 locked Windows packages |
| PowerShell 7.5.4 parser on Linux | Parsed 74 scripts/fenced examples; README's seven core commands passed against disposable storage, four provider examples used help only |
| Linux pseudo-terminal menu checks | Styled case passed; `NO_COLOR`, empty `NO_COLOR`, `TERM=dumb` and C locale each passed with ASCII and no ANSI; q exited 0 without creating data |
| Windows MSVC target type checking | `display.rs` and the private-console fixture passed metadata compilation; no Windows linking or execution |

Machine-readable local evidence is in
[`windows-terminal-linux-2026-10-23.json`](evidence/windows-terminal-linux-2026-10-23.json).

## Remaining validation

No Windows executor was available. The new Windows shell/console suite, existing
Windows package/PE/recipient checks, and fresh static-runtime producer have **not
run** for this working tree. Run the provisioned Windows deterministic gate and
the [fresh producer](RELEASE_PREPARATION.md) before Windows acceptance:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1 -DeterministicOnly -ReleaseTests
```

The console fixture's simulated Windows Terminal marker exercises program logic
on real console handles. It cannot establish physical font rendering, actual
Windows Terminal behavior or clean-profile/other-PC compatibility. Those still
need Windows Terminal and legacy Console visual checks. Unknown console fonts
deliberately receive ASCII; arbitrary glyph coverage cannot be inferred from a
shell name or code page. The installer needs Windows PowerShell 5.1 for its CMD
entry point; direct PowerShell 7 installation remains available.

These local results were collected before any commit or push, without CI dispatch
or publication. Subsequent hosted checks are tracked on the pull request.

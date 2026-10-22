# SwapCtx Project Roadmap & Community Priorities

SwapCtx remains a Windows-first Rust CLI with local SQLite persistence, deliberate
memory capture, and explicit provider sharing. No mandatory cloud service, background daemon, or subscription is planned. This roadmap outlines core priorities and contribution opportunities.

---

## 🎯 Current Milestone: First Public Preview (v0.1.0-preview)

New features are frozen for v0.1.0-preview. Focus on release-blocking fixes, evidence and documentation. Before publication, complete the build reproducibility investigation [#13](https://github.com/rehnx/SwapCtx/issues/13), private intake/publisher trust [#5](https://github.com/rehnx/SwapCtx/issues/5), and independent Windows acceptance [#6](https://github.com/rehnx/SwapCtx/issues/6).

Key objectives before issuing the initial public preview:

- **Unified Native & Intelligence Foundation**: Ensure deterministic test coverage across Windows and Linux runner environments.
- **Handoff & Memory Diagnostics**: Keep memory handoff (`swapctx handoff`) and health audit (`swapctx memory audit`) contracts intuitive, transparent, and reviewable without automatic knowledge merging.
- **Distribution Security & Integrity**: Verify payload checksums, release candidate packages (`install-windows.ps1`), and establish verified security contact channels.
- **Release Documentation**: Finalize trusted installation guides, CLI command reference, and contributor onboarding documentation.

---

## 🛠️ Contributor Focus Areas (after the preview freeze)

We welcome community contributions! The following areas are great entry points for new contributors:

### 1. Developer Experience & CLI Usability
- **Diagnostic Clarity**: Improve error messages and suggestions in `swapctx doctor` and `swapctx status`.
- **Shell Completion & Formatting**: Help refine CLI help texts, table formatting, and shell integration documentation.
- **Workflow Examples**: Contribute real-world workflow patterns and tutorial guides for prompt engineering with persistent local context.

### 2. Testing & Synthetic Fixtures
- **Deterministic Test Coverage**: Add synthetic lifecycle fixtures in `tests/` to verify edge cases in memory ranking, byte budget enforcement, and protocol serialization.
- **Fake Provider Scenarios**: Expand mock provider responses in integration test scripts (`scripts/check-native.cjs`) to cover additional lifecycle callbacks (e.g. session compaction, fork, resume).

### 3. Community Documentation & Translation
- **Troubleshooting Scenarios**: Expand [SUPPORT.md](../SUPPORT.md) with common setup issues, PATH configuration edge cases, and Node 24+ version checks.
- **Protocol Guides**: Improve technical explanations of `swapctx context --protocol-version 1` JSON protocols in [docs/CONTEXT_PROTOCOL.md](CONTEXT_PROTOCOL.md).

---

## 🚀 Post-Preview Engineering Goals

- **Live Provider Evaluation**: Evaluate exact installed Codex and Claude versions in separately authorized live sessions to distinguish hook loading, local preparation, actual context receipt, and model adherence.
- **Adapter Ecosystem**: Support additional coding agent interfaces while keeping provider processes, authentication, and network communication cleanly separated from the core local memory engine.
- **Performance & Large Stores**: Benchmarks and optimizations for SQLite retrieval performance on projects with thousands of active memory records.

---

Track acceptance work in the [preview milestone](https://github.com/rehnx/SwapCtx/milestone/1).
See [launch preparation](LAUNCH_READINESS.md) and [unified integration](UNIFIED_INTEGRATION.md) for current evidence and decision checklists.

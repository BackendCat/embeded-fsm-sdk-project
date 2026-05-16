# Changelog

All notable changes to the **FSM Studio for VS Code** extension are
documented here. This changelog covers the *extension* only; the bundled
FSM-Lang toolchain (compiler, verifier, code generator) is versioned
separately.

The format follows [Keep a Changelog](https://keepachangelog.com/);
the extension uses [Semantic Versioning](https://semver.org/).

## [1.0.0]

First Marketplace-ready release. The editor surface is feature-complete and
the package is publish-valid.

### Added

- **Formal verification in the editor.** New commands
  `FSM Studio: Verify (Deadlock-Freedom + Reachability)` (`fsm.verify`),
  `FSM Studio: Verify Live (via Language Server)` (`fsm.verifyLive`), and
  `FSM Studio: Baseline Regression Check` (`fsm.baseline`). The verdict,
  exploration stats, the deadlock counterexample witness (navigable to
  source), and reachability findings (as editor diagnostics) are surfaced.
  An inconclusive result is always reported as inconclusive, never as
  "verified".
- **Discoverable codegen settings.** `fsmLang.codegen.outputDir`,
  `fsmLang.codegen.strategy`, `fsmLang.codegen.queueSize`,
  `fsmLang.codegen.license`, `fsmLang.codegen.emitIr`, and
  `fsmLang.codegen.reportMemory` now appear in Settings and are honored by
  the Generate commands. (`outputDir` and `strategy` were already honored
  but were previously undiscoverable.)
- Extension icon and Marketplace keywords.

### Changed

- The Generate commands now pass the new codegen settings through to the
  bundled `fsm` CLI when they diverge from its defaults.

### Fixed

- The diagram webview's Content-Security-Policy nonce is now sourced from a
  cryptographically-secure RNG (`crypto.randomBytes`) instead of
  `Math.random()`.
- Corrected the extension's repository URL to the project's actual
  repository.

## [0.1.x] — pre-release (bundled with the toolchain's v1.3–v1.4 line)

Initial editor surface, not published to the Marketplace:

- FSM-Lang syntax highlighting, snippets, and language configuration.
- Language-server client: live diagnostics, document symbols, hover, and
  configurable inlay hints.
- Machines and Events activity-bar explorers.
- Read-only ELK diagram panel with click-to-source navigation and an honest
  stale-render banner.
- Code-generation, format, check, and copy-IR commands shelling the bundled
  `fsm` CLI, with verbatim error surfacing (no fake successes).
- Host-platform `fsm` / `fsm-lang-server` binaries bundled into the package.

# Phase 5: Public v0.1 Release

## Context

The accepted desktop workflows now cover Codex Skill discovery, editing,
creation, lifecycle management, candidate review, Deep Audit, and Bundle
migration. Phase 5 turns that local development application into a public,
non-programmer-friendly macOS release without weakening its privacy or mutation
boundaries.

Task 4.6 remains a release gate: the owner still needs to validate one real
Mac-to-Linux Bundle migration. Entering Phase 5 does not manufacture that
missing evidence or mark the migration accepted.

## Scope

- Complete accessibility, dark-mode, reduced-motion, reduced-transparency, and
  native-window visual QA for the common workflows.
- Add complete Simplified Chinese and English interface localization with a
  persistent language setting.
- Establish reproducible CI and release checks.
- Configure macOS signing, notarization, packaging, and an evidence-based
  minimum supported macOS version.
- Publish user, contributor, privacy, security, migration, and limitation
  documentation with the MIT-licensed source repository.
- Validate installation, launch, core workflows, upgrade behavior, and removal
  on a clean supported Mac before any public release is published.

## Non-Goals

- Windows or Linux desktop packages.
- Additional Agent adapters or non-Skill capability modules.
- Automatic updates, telemetry, accounts, cloud synchronization, or a hosted
  service.
- Committing Apple signing credentials, notarization credentials, generated
  packages, local audit state, or exported Skill Bundles to the source tree.
- Publishing a GitHub repository or release without the owner's explicit final
  authorization.

## Decisions And Alternatives

### Repository document policy

Keep the following documentation in the public repository:

- `README.md` and `LICENSE` for users and license compliance.
- `AGENTS.md`, `INIT.md`, `CONTEXT.md`, and `PLAN.md` for contributor context,
  durable constraints, domain language, and roadmap transparency.
- `docs/adr/`, `docs/phases/`, and `docs/research/` for architectural rationale,
  accepted milestone evidence, existing-solution validation, and threat-model
  traceability.
- `docs/server-migration.md` as a user-facing migration guide.

These files contain product and engineering evidence rather than credentials or
private user data. A pre-publication scan must still reject local absolute
paths, usernames, hostnames, server addresses, credentials, tokens, private
keys, and accidental generated reports.

Do not use `.gitignore` as a publication policy for documents. It does not stop
already tracked files from being published. Keep `.gitignore` focused on
dependencies, build output, local application state, visual-QA artifacts,
editor files, environment files, and signing material. Signed installers belong
in release assets, not Git history.

### Localization boundary

Do not expose a language selector until the complete common path changes
consistently. Use stable message identifiers and one frontend localization
catalog for visible WebView text. Backend commands should return typed or stable
error identities where the frontend needs localization; exact technical evidence
such as paths, hashes, model names, and provider endpoints remains untranslated.
Persist only the selected interface language, never infer a change that could
surprise the user after an update.

### Release trust

Local development builds may remain unsigned. A public v0.1 must use Developer
ID signing, Apple notarization, and stapling. Credentials live only in the
owner's Keychain or the release platform's encrypted secret store. CI may build
and test unsigned pull requests, but only an explicitly authorized release job
may access signing credentials or publish artifacts.

## Task Breakdown

### Task 5.1 - Accessibility, Appearance, And Localization

- Status: accepted by the first user on 2026-08-08 after automated verification
  and native-window acceptance.
- Outcome: the complete common workflow is readable and operable in light and
  dark appearance, with keyboard and assistive-technology basics, and can switch
  consistently between Simplified Chinese and English.
- Affected files: `index.html`, `src/app.js`, `src/styles.css`, focused frontend
  state/localization modules and tests, and Tauri window settings where required.
- Key design: inventory every user-visible string and backend-originated error;
  introduce stable localization keys; persist the chosen language locally; keep
  focus order, focus restoration, labels, live regions, contrast, text fitting,
  reduced motion, and reduced transparency verifiable. Put Settings and language
  selection in the native application menu, with `Command+,` opening Settings.
- Automated verification: localization-key parity, missing-key failure, language
  persistence, dialog focus behavior where testable, production frontend build,
  frontend tests, Rust tests, and warnings-denied Clippy.
- Human verification: the first user accepted the live Tauri workflow, native
  Settings and language menus, `Command+,`, password-free Settings opening,
  appearance behavior, and credential persistence on 2026-08-08.

### Task 5.2 - CI, Release Metadata, Signing, And Packaging

- Status: automated implementation complete; signed candidate and human
  verification pending.
- Outcome: clean commits run deterministic checks, and an authorized release can
  produce a signed, notarized, stapled macOS installer with traceable version and
  checksum evidence.
- Affected files: `.github/workflows/`, Tauri configuration, package metadata,
  release scripts or configuration, and release documentation.
- Key design: pin or lock the Node and Rust toolchains; use clean dependency
  installation; separate untrusted pull-request checks from credential-bearing
  release jobs; set and verify the minimum macOS version; never print secrets.
- Automated verification: frontend tests/build, locked Rust tests/checks, Clippy
  with warnings denied, Bundle fixtures, release build, signature inspection,
  notarization result, staple validation, and artifact checksums.
- Human verification: install and launch the exact release candidate outside the
  build tree without Gatekeeper bypass instructions.

Implementation record (2026-08-17):

- `.nvmrc` pins Node.js 22.23.1 and `rust-toolchain.toml` pins Rust 1.88.0.
  The Cargo lockfile contains dependencies whose published MSRV is 1.88, so the
  release pin was raised from the project's general Rust 1.85-or-later floor
  instead of claiming a CI configuration that cannot build.
- `release:check` verifies version, repository, MIT metadata, bundle targets,
  the packaged macOS icon, macOS 13.0, hardened runtime, exact toolchain pins,
  and (for the candidate) the checked-out version tag.
- Pull-request CI has no credential access and builds an unsigned universal
  `.app` and `.dmg`. The protected manual candidate workflow requires the
  `release` environment, writes the App Store Connect `.p8` key with mode 0600,
  runs Tauri signing/notarization/stapling, verifies Developer ID signatures,
  Gatekeeper, and checksums, then uploads a temporary candidate artifact without
  publishing a GitHub Release.
- Automated evidence on the implementation revision: 61 frontend tests, 164
  desktop-core tests, 24 Bundle-core tests, production frontend build, locked
  Cargo check, warnings-denied Clippy, YAML parsing, and a successful universal
  unsigned `.app`/`.dmg` build whose metadata verifier reported version 0.1.0,
  macOS 13.0, x86_64+arm64, and a packaged `icon.icns`. The application icon,
  packaged app icon, and DMG volume icon had the same SHA-256. The verifier
  mounted the DMG read-only and matched its complete application directory
  revision (`de41a9fe741ba74bf74d4159816b96a0cfb5e180135abe9473ccc2330d424a81`)
  to the loose application that passed metadata checks.
- A signed Apple candidate was not run in this workspace because the protected
  environment's certificate and App Store Connect credentials were not supplied.
  Do not treat signing, notarization, stapling, Gatekeeper, or clean-machine
  installation as passed. Task 5.2 remains open until the exact signed candidate
  installs and launches outside the build tree; Task 5.4 records the broader
  clean-machine workflow and publication evidence.

### Task 5.3 - Public Documentation And Repository Readiness

- Status: pending.
- Outcome: a non-programmer can understand the product boundary, install and use
  the app, understand cloud-data behavior and audit limitations, migrate Skills,
  report a security issue, and inspect the source license.
- Affected files: `README.md`, `LICENSE`, `SECURITY.md`, `PRIVACY.md`, contributor
  guidance if contributions are opened, `README.zh-CN.md`, screenshots, and
  existing `docs/` files.
- Key design: keep the common path concise; separate user documentation from
  contributor evidence; keep English `README.md` as the GitHub default with a
  visible Simplified Chinese link and a complete `README.zh-CN.md` that links
  back to English; publish no machine-specific data; state that Audit is
  evidence, not a security guarantee; document exactly when files leave the Mac.
- Automated verification: link checks, repository secret/path scan, license and
  metadata consistency, and screenshot asset validation.
- Human verification: a non-programmer follows the README without requiring a
  terminal for application installation or ordinary use.

### Task 5.4 - Clean-Machine Acceptance And Publication

- Status: pending.
- Outcome: the exact v0.1 release candidate passes clean-machine acceptance and
  is published only after the owner reviews the artifact, repository contents,
  known limitations, and release notes.
- Dependencies: Tasks 4.6 and 5.1-5.3 completed; signing/notarization authority
  available; final repository and release destination confirmed by the owner.
- Automated verification: rerun the release suite from the tagged revision and
  match the published artifact checksum to the accepted candidate.
- Human verification: install, first launch, catalog discovery, edit/save,
  lifecycle, candidate intake, Bundle migration, optional provider configuration,
  upgrade/reinstall, and uninstall on a clean supported Mac.

## Risks And Mitigations

- Partial localization: fail tests on missing keys and do not expose the selector
  before both catalogs cover the common path.
- Inaccessible custom controls: prefer native elements, preserve visible focus,
  and verify keyboard and VoiceOver behavior in the native window.
- Signing-secret exposure: isolate release credentials from pull requests, logs,
  repository files, and developer fixtures.
- Publishing private machine data: run content and history scans before making
  the repository public; manually inspect screenshots and documentation.
- Release/configuration drift: derive version and checks from one tagged source
  revision and verify the installed artifact rather than only the build folder.
- Unproven migration claim: keep Task 4.6 as a blocking release acceptance item
  until the owner supplies a real Linux-server result.

## Verification Matrix

| Area | Automated | Human |
| --- | --- | --- |
| Localization | key parity, persistence, build/tests | both languages across common workflows |
| Accessibility | static semantics and focused state tests | keyboard, VoiceOver, contrast, text fit |
| Appearance | CSS/build checks | light/dark and reduced effects in native window |
| Release | CI, signature, notarization, staple, checksum | install and launch exact candidate |
| Documentation | links, secret/path scan, metadata | non-programmer walkthrough |
| Migration | existing Bundle suites | real Mac-to-Linux Task 4.6 pass |

## Acceptance Record

- The owner authorized entering Phase 5 on 2026-08-06.
- Phase 5 implementation requires the detailed plan to remain aligned with the
  accepted v0.1 boundary. Publication itself remains a separate explicit owner
  action.
- Task 5.1 implementation on 2026-08-08 added a persistent Simplified
  Chinese/English localization module with catalog-parity, duplicate-key, static
  marker, and literal-call coverage tests. The common desktop path, stable error
  codes, built-in finding titles, compatibility labels, Bundle decisions, and
  trusted server instruction now follow the selected interface language. The
  native application menu owns Settings and language selection.
- The owner replaced session-only and Keychain credential modes with the
  passwordless local store in ADR 0002. Provider preferences persist separately;
  API-key files use strict current-user permissions and atomic replacement, and
  opening Settings checks only credential-file metadata. This deliberately does
  not claim protection from software already running as the same macOS account.
- The same pass linked dialogs and tab panels to their accessible labels,
  strengthened visible keyboard focus, preserved reduced-motion/transparency
  and high-contrast behavior, and fixed a localization regression where the
  `仅本机` status text inherited the status-dot dimensions and rendered one
  character per line.
- Automated verification covers 21 frontend tests, 110 desktop-core tests, 24
  Bundle-core tests, the production frontend build, warnings-denied workspace
  Clippy, and diff checks. The production JavaScript chunk is about 550 kB
  uncompressed and 133 kB gzip after carrying both locale catalogs; Vite reports
  its advisory 500 kB chunk warning, but no release-size or runtime failure.
- Native visual automation was unavailable because the in-app browser connection
  could not be established and macOS screen capture could not read the display.
  The owner completed the required live Tauri acceptance on 2026-08-08.

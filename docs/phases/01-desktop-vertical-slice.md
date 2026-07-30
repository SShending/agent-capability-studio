# Phase 1: Native Desktop Vertical Slice

## Status

Accepted by the project owner. Task 1.0 is in progress.

## Context

The current Node prototype proves catalog scanning, guided/source editing,
baseline evidence, line comparison, optimistic concurrency, atomic save, and the
macOS-oriented interaction model. It still requires a local HTTP server and
Node.js, so it is not the accepted desktop product.

This phase proves the Tauri/Rust/WebView architecture through one complete user
workflow before porting creation, candidate installation, archive management, or
bundle migration.

## Scope

- Tauri 2 macOS application shell and development build.
- Static frontend build using the existing HTML/CSS/JavaScript behavior.
- Rust Codex catalog discovery and detail reading.
- Rust baseline audit, diff, hash conflict detection, and atomic `SKILL.md` save.
- Typed frontend bridge for the vertical workflow.
- Native-window visual and human acceptance.
- Removal of the runtime Node server only after parity is demonstrated.

## Non-Goals

- Creating new Skills.
- GitHub or local candidate acquisition and installation.
- Enable/disable, archive/restore, and bulk operations unless required to prove
  the command design.
- Skill Bundle import/export.
- External scanner execution.
- Other Agent Adapters or operating systems.
- Signing and notarization; those belong to the public release phase.

## Decisions And Alternatives

### Desktop shell

Use Tauri 2. It satisfies the accepted lightweight desktop direction and avoids
shipping a full browser runtime. Electron is rejected for v0.1 because the
product does not need its larger runtime or Node integration.

### Frontend migration

Keep vanilla JavaScript for the vertical slice and add a small build step for
module imports and Tauri integration. Do not combine the desktop migration with a
framework rewrite. Reassess only if guided creation exposes state complexity that
the current approach cannot manage cleanly.

### Command seam

Expose a narrow interface around user outcomes rather than filesystem
primitives:

- `list_skills`
- `get_skill`
- `audit_draft`
- `save_draft`

Do not expose generic read/write-path commands to the frontend. Transport types
must include stable IDs, source ownership, content hashes, evidence, and explicit
error codes for user-recoverable conflicts.

### Port strategy

Port and parity-test behavior before deleting the Node implementation. Avoid
maintaining two production engines: after acceptance, remove the runtime server
and keep only useful cross-language fixtures or oracle cases.

## Task Breakdown

### Task 1.0 - Establish A Recoverable Local Baseline

- Status: pending
- Outcome: the accepted documents and working prototype have a clean local Git
  baseline before files are moved or deleted.
- Details:
  - create a focused `.gitignore` for dependencies, build artifacts, generated
    screenshots, local state, and signing material;
  - initialize a local Git repository;
  - inspect every tracked file and exclude secrets or machine-specific data;
  - record the current accepted prototype and planning documents in a baseline
    commit after human review;
  - do not create a GitHub repository, remote, release, or push without separate
    explicit approval.
- Affected files:
  - `.gitignore`
  - local `.git/` metadata
- Key design: preserve the current prototype as a recoverable reference while
  preventing `node_modules`, generated output, credentials, and future signing
  material from entering version control.
- Dependencies: accepted Phase 1 plan.
- Automated verification:
  - ignored dependency and build paths do not appear in `git status`;
  - tracked files contain no obvious credential or secret material;
  - the baseline commit is readable and the working tree is clean.
- Human verification:
  - review the exact baseline file list and commit message before any remote
    publication.

### Task 1.1 - Scaffold Tauri And Frontend Build

- Status: pending
- Outcome: the existing interface opens in a native Tauri development window.
- Details:
  - create the standard Tauri 2 Rust application and capabilities;
  - introduce a minimal frontend build that bundles Lucide and static assets;
  - move frontend sources into a clear source layout without changing behavior;
  - configure macOS window identity, title, dimensions, and drag regions;
  - keep permissions minimal and document why each capability is needed.
- Affected files:
  - `package.json`, `package-lock.json`
  - `index.html`
  - `src/app.js`, `src/styles.css`, `src/desktop-bridge.js`
  - `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`
  - `src-tauri/capabilities/default.json`
  - `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
  - Tauri application icons
- Key design: the initial desktop bridge may report unsupported commands, but the
  UI must load without the Node server and without direct filesystem access.
- Dependencies: Tauri 2 CLI and build dependencies; no frontend framework.
- Automated verification:
  - frontend production build succeeds;
  - `cargo check` succeeds;
  - Tauri configuration validation succeeds.
- Human verification:
  - native window opens with correct title, minimum size, dark mode, and keyboard
    focus;
  - no browser tab or separate terminal-launched server is required after launch.

### Task 1.2 - Implement The Rust Codex Catalog

- Status: pending
- Outcome: the native app lists and inspects real Codex Skills through Rust.
- Details:
  - discover personal, disabled, system, plugin-managed, and archived roots;
  - parse Skill metadata and optional UI metadata without executing content;
  - classify ownership, trigger strategy, and editability;
  - use stable IDs without accepting arbitrary frontend paths;
  - deduplicate plugin cache revisions with documented selection semantics;
  - expose precise, user-readable errors while retaining machine error codes.
- Affected files:
  - `src-tauri/src/skills/mod.rs`
  - `src-tauri/src/skills/catalog.rs`
  - `src-tauri/src/skills/types.rs`
  - `src-tauri/tests/fixtures/catalog/`
  - `src/desktop-bridge.js`, `src/app.js`
- Key design: Rust resolves configured Codex roots and maps opaque IDs to
  contained paths. The frontend never supplies a path for catalog reads.
- Dependencies: select a maintained YAML parser after checking maintenance and
  test quality; avoid an ad hoc frontmatter parser in the final core.
- Automated verification:
  - fixtures cover every ownership source, missing roots, malformed frontmatter,
    plugin duplicates, icons, Unicode, and symlinks;
  - Rust tests assert system/plugin read-only classification;
  - parity fixture compares the accepted Node catalog behavior where applicable.
- Human verification:
  - native catalog matches the user's actual Codex collection and source counts;
  - restored contextual-trigger Skills display as contextual, not defective.

### Task 1.3 - Port Audit, Diff, And Guarded Save

- Status: pending
- Outcome: one personal Skill can be edited and safely saved without Node.
- Details:
  - port the evidence model and baseline checks into the Rust core;
  - treat explicit-name and contextual-intent triggers as legitimate strategies;
  - return a deterministic diff against the exact opened revision;
  - reject system/plugin writes, symlink targets, containment escapes, malformed
    drafts, blocking findings, and stale content hashes;
  - write a same-directory temporary file and atomically replace `SKILL.md`;
  - preserve recoverable error details for the UI.
- Affected files:
  - `src-tauri/src/skills/audit.rs`
  - `src-tauri/src/skills/diff.rs`
  - `src-tauri/src/skills/workspace.rs`
  - `src-tauri/src/skills/types.rs`
  - `src-tauri/tests/fixtures/workspace/`
  - `src/desktop-bridge.js`, `src/app.js`
- Key design: `audit_draft` is pure and side-effect free. `save_draft` repeats
  server-side validation and requires the opened content hash.
- Dependencies: Rust hashing and temporary-file facilities selected for active
  maintenance and macOS atomic-rename behavior.
- Automated verification:
  - tests cover evidence severity, contextual triggers, high-impact commands,
    empty/malformed documents, hash conflicts, path containment, symlinks,
    read-only sources, cleanup after failed writes, and successful atomic save;
  - mutation tests use temporary fixtures, never live Skills.
- Human verification:
  - edit a disposable personal Skill, inspect evidence and diff, save it, reopen
    it, and confirm exact content;
  - create an external concurrent edit and confirm the app refuses overwrite;
  - cancel a changed draft and confirm no file mutation.

### Task 1.4 - Connect And Consolidate The Desktop UI

- Status: pending
- Outcome: the native interface completes the vertical workflow with coherent
  loading, error, empty, review, and confirmation states.
- Details:
  - replace HTTP `fetch` calls and CSRF handling with the desktop bridge;
  - keep source ownership and exact evidence visible in the Inspector;
  - preserve draft debounce without hiding audit-in-progress state;
  - map recoverable Rust error codes to specific plain-language actions;
  - consolidate the prototype stylesheet instead of layering another override;
  - preserve dark mode, reduced motion, reduced transparency, contrast, keyboard
    focus, and spatially consistent editor transitions.
- Affected files:
  - `src/app.js`, `src/styles.css`, `src/desktop-bridge.js`, `index.html`
  - frontend checks and UI fixtures
- Key design: the desktop bridge is the only frontend dependency on Tauri. UI
  rendering and form logic remain testable with a fake bridge.
- Automated verification:
  - frontend syntax/build checks pass;
  - bridge tests cover success and typed error mapping;
  - all referenced DOM IDs and accessible names resolve.
- Human verification:
  - inspect desktop widths and a narrow window without overlap or clipped text;
  - keyboard-only completion of open, edit, audit, save, and discard;
  - light/dark and accessibility preference checks;
  - screenshots show nonblank content, correct Inspector framing, and no overlap.

### Task 1.5 - Prove Parity And Build The Local App

- Status: pending
- Outcome: the vertical slice runs from a local `.app` with no Node runtime.
- Details:
  - run Node and Rust implementations against shared fixtures and resolve
    meaningful differences;
  - remove HTTP server, CSRF token, runtime Node child process assumptions, and
    obsolete server-only UI paths after parity;
  - keep or translate valuable tests before deleting duplicate implementation;
  - build the unsigned local macOS `.app`;
  - update README for the desktop development workflow and current limitations.
- Affected files:
  - `server.mjs`, `skill-workspace.mjs`, `test/server.test.mjs`
  - `package.json`, `README.md`
  - Rust/frontend parity fixtures and build configuration
- Key design: deletion occurs only after accepted behavior is covered in the new
  core. Do not retain a hidden Node fallback in the shipped app.
- Automated verification:
  - all Rust and frontend tests pass;
  - production frontend and Tauri builds pass;
  - built application contains no Node runtime or server launch command;
  - live personal Skills are not modified by automated tests.
- Human verification:
  - launch the `.app` from Finder;
  - complete the disposable-Skill vertical workflow;
  - quit and relaunch without a terminal process;
  - confirm the product reports limitations honestly.

## Risks And Mitigations

- **Parser behavior changes**: use shared fixtures and inspect real Skills before
  deleting the Node implementation.
- **Rust port expands into a rewrite**: enforce the four-command vertical scope;
  defer archive, install, creation, and bundle commands.
- **Live Skill damage**: automated tests use temp roots; human tests use a named
  disposable Skill and a recorded backup.
- **Symlink and atomic-write differences**: test macOS filesystem behavior
  explicitly and fail closed for unsupported targets.
- **UI redesign churn**: preserve interaction behavior first, then consolidate
  styles within Task 1.4 with screenshot comparison.
- **Tauri permissions become broad**: expose only application commands and keep
  capabilities minimal; never grant generic shell or filesystem access to the
  WebView.

## Verification Matrix

| Requirement | Automated evidence | Human evidence |
| --- | --- | --- |
| No Node runtime | bundle/config inspection | Finder launch without server |
| Correct catalog | Rust fixtures and parity cases | compare actual Codex sources |
| Read-only managed Skills | command tests | managed Skill has no edit action |
| Side-effect-free audit | filesystem snapshot tests | audit leaves candidate unchanged |
| Guarded save | conflict/symlink/atomic tests | disposable Skill save and conflict |
| Understandable evidence | stable result fixtures | first-user review of findings |
| Accessible desktop UI | build/DOM/bridge checks | keyboard, dark mode, reduced motion |

## Acceptance Record

- Plan accepted by human: pending
- Automated verification: pending
- Human workflow verification: pending
- Phase accepted: pending

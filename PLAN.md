# Agent Skill Studio Plan

## Current Position

- Active phase: Phase 5 public v0.1 release
- Current task: 5.2 add CI, release metadata, signing, notarization, and packaging
- Next task: 5.3 reconcile public documentation, screenshots, and limitations
- Last accepted milestone: Task 5.1 accessibility, appearance, localization,
  native menu, and passwordless local credential storage, accepted by the owner
  on 2026-08-08
- Blocking decision: none; the owner simplified server migration on 2026-08-06
  to direct Codex-assisted installation of a trusted Mac export, with no custom
  headless Bundle CLI or repeated semantic audit
- Validation item: confirm the final minimum macOS version before public release
- Outstanding acceptance: Task 4.6 real Mac-to-Linux migration remains a
  release gate; it has not been marked complete without a real server result
- Human validation: Task 5.1 passed first-user native-window acceptance on
  2026-08-08

## Phase 1 - Native Desktop Vertical Slice

Status: accepted by the project owner on 2026-07-30.

Outcome: a Tauri 2 macOS application completes one real Codex workflow without
the Node HTTP server: discover, inspect, edit, audit, compare, and guarded-save a
personal Skill.

Detailed plan: [01-desktop-vertical-slice.md](docs/phases/01-desktop-vertical-slice.md)

Acceptance:

- A local `.app` opens without Node.js or terminal setup.
- The app discovers the real Codex catalog and preserves source ownership.
- Guided/source edit, baseline audit, diff, conflict detection, save, and discard
  work through typed Tauri commands.
- System and plugin-managed Skills remain read-only.
- Rust and frontend checks pass, and a human validates the workflow in the native
  window.

Tasks:

- [x] 1.0 Establish a recoverable local Git baseline. Local-only baseline: `9eb75c5`.
- [x] 1.1 Scaffold Tauri 2 and the frontend build.
- [x] 1.2 Implement the read-only Codex catalog in Rust.
- [x] 1.3 Port audit, diff, and guarded save into Rust.
- [x] 1.4 Connect the frontend through a typed desktop bridge and consolidate UI.
- [x] 1.5 Remove Node runtime dependency after parity and build the local `.app`.

## Phase 2 - Guided Skill Creation And Personal Lifecycle

Status: accepted by the project owner on 2026-07-31.

Detailed plan: [02-guided-skill-creation.md](docs/phases/02-guided-skill-creation.md)

Outcome: a non-programmer creates and manages a valid personal Codex Skill
without editing files directly.

Acceptance:

- The flow captures purpose, trigger strategy, workflow, and supporting files.
- The draft is audited and previewed before creation.
- Audit wording distinguishes a limited baseline check from a security
  guarantee, and high-confidence blockers are verified against adversarial and
  benign fixtures.
- Optional Deep Audit uses a user-configured cloud model only after showing the
  endpoint and exact files that will leave the machine.
- Name/path collisions, invalid files, and concurrent changes are handled without
  overwriting existing Skills.
- The created Skill is immediately discoverable and editable.
- Personal Skills can be disabled, re-enabled, archived, restored, and
  permanently deleted through guarded actions; system and plugin Skills remain
  read-only.
- Lifecycle actions avoid repeated full-catalog scans while retaining separate
  preview-time and apply-time checks of the affected directory revision.

Tasks:

- [x] 2.1 Define the Skill Draft document model and heading-aware form flow.
- [x] 2.2 Implement safe Rust creation commands and collision handling.
- [x] 2.3 Add draft preview, audit, confirmation, and post-create navigation.
- [x] 2.4 Run unified creation, safety, lifecycle, and recovery acceptance after Tasks 2.5, 2.6, and 2.7.
- [x] 2.5 Harden built-in safety evidence and validate scanner integration options.
- [x] 2.6 Implement guarded personal Skill lifecycle actions.
- [x] 2.7 Eliminate lifecycle catalog rescans and verify responsive transitions.

## Phase 3 - GitHub And Local Candidate Audit

Status: accepted by the project owner on 2026-08-04.

Detailed plan: [03-github-local-candidate-audit.md](docs/phases/03-github-local-candidate-audit.md)

Outcome: a user submits a public GitHub Skill or local directory, reviews exact
evidence and files in staging, and separately confirms installation.

Acceptance:

- GitHub candidates record repository, path, ref, and resolved commit SHA.
- Local and GitHub acquisition never executes candidate scripts.
- Audit is side-effect free and installation requires a separate confirmation.
- Findings can use the built-in evidence model and an external-scanner adapter
  seam without requiring a scanner installation.

Tasks:

- [x] 3.1 Validate the safest maintained GitHub acquisition approach. Research:
  [github-acquisition-options.md](docs/research/github-acquisition-options.md).
- [x] 3.2 Implement contained temporary staging for GitHub and local candidates.
- [x] 3.3 Present files, hashes, compatibility, findings, and exact version.
- [x] 3.4 Implement explicit installation with conflict and destination checks.
- [x] 3.5 Add consent-bound cloud Deep Audit for staged candidates.
- [x] 3.6 Define and test the external scanner evidence adapter interface.

## Phase 4 - Skill Bundle Migration

Detailed plan: [04-skill-bundle-migration.md](docs/phases/04-skill-bundle-migration.md)

Outcome: eligible personal Skills move between machines through a versioned,
hash-verified bundle with staging and conflict review.

Acceptance:

- Export includes only user-controlled Skills and a versioned manifest with every
  file hash.
- Import rejects traversal, unsafe entries, malformed manifests, and hash
  mismatches before writing outside staging.
- Identical Skills are skipped; new Skills are candidates; same-name differences
  require comparison and confirmation; system/plugin conflicts are blocked.
- Import does not install until the user explicitly confirms target Skills.

Tasks:

- [x] 4.1 Specify and fixture-test the versioned Skill Bundle format.
- [x] 4.2 Implement safe export with eligibility and secret checks (accepted by
  the project owner on 2026-08-05).
- [x] 4.3 Implement contained parsing, verification, and staging (accepted by
  the project owner on 2026-08-05).
- [x] 4.4 Add conflict classification, diff review, and explicit installation
  (accepted by the project owner on 2026-08-06).
- [x] 4.5 Document and validate direct Codex-assisted installation of a trusted
  Mac export on the owner's Linux server, using existing transfer tools and no
  repeated semantic audit or custom CLI. The export receipt provides a tested
  copyable server instruction (desktop handoff accepted by the project owner on
  2026-08-06; real server validation remains in Task 4.6).
- [ ] 4.6 Verify the real Mac-to-Linux migration, including identical skips and
  explicit handling of different same-name server content.

## Phase 5 - Public v0.1 Release

Detailed plan: [05-public-v01-release.md](docs/phases/05-public-v01-release.md)

Outcome: a documented, signed, notarized macOS release suitable for
non-programmers and an MIT-licensed public repository.

Acceptance:

- The final supported macOS version is evidence-based and documented.
- Release builds are signed, notarized, and packaged for normal macOS install.
- A clean machine can install, launch, complete the core workflows, and uninstall.
- Privacy, audit limitations, recovery behavior, and product boundaries are
  documented plainly.
- CI verifies Rust, frontend, bundle fixtures, and release packaging checks.

Tasks:

- [x] 5.1 Complete accessibility, dark-mode, native-window visual QA, and real
  Simplified Chinese/English localization with a persistent interface-language
  setting. Do not expose a language selector until all common-path strings and
  error messages switch consistently.
- [ ] 5.2 Add CI, release metadata, signing, notarization, and packaging.
- [ ] 5.3 Reconcile README, license, product screenshots, and limitations.
- [ ] 5.4 Run clean-machine acceptance and publish the v0.1 artifacts.

## Deferred Roadmap

- Claude Code, OpenClaw, and Hermes Agent Adapters.
- MCP configuration explanation and launch/integration with MCP Inspector.
- Rules, plugins, hooks, and automation authoring.
- Runtime tool-call observability with explicit privacy and retention design.
- Windows and Linux packaging after macOS and adapter contracts are stable.

## Post-v0.1 Capability Validation Gate

Before adding a non-Skill module:

1. Recheck current products and open-source implementations for that capability.
2. Select exactly one second capability type for a small vertical slice.
3. Prove it reuses ownership, evidence, comparison, guarded mutation, and Agent
   Adapter concepts without weakening the Skill model.
4. Integrate or launch maintained specialist tools instead of rebuilding them.
5. Validate the workflow with the first user before scheduling further modules
   or reconsidering the product name.

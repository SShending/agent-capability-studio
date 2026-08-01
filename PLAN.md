# Agent Skill Studio Plan

## Current Position

- Active phase: Phase 3 preparation
- In-progress task: none
- Next task: 3.1 Validate the safest maintained GitHub acquisition approach
- Last accepted milestone: Phase 2 guided creation and personal lifecycle
- Blocking decision: select the Phase 3 acquisition approach through Task 3.1
  before implementing candidate staging
- Validation item: confirm the final minimum macOS version before public release

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
- [ ] 3.2 Implement contained temporary staging for GitHub and local candidates.
- [ ] 3.3 Present files, hashes, compatibility, findings, and exact version.
- [ ] 3.4 Implement explicit installation with conflict and destination checks.
- [ ] 3.5 Define and test the external scanner evidence adapter interface.

## Phase 4 - Skill Bundle Migration

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

- [ ] 4.1 Specify and fixture-test the versioned Skill Bundle format.
- [ ] 4.2 Implement safe export with eligibility and secret checks.
- [ ] 4.3 Implement contained parsing, verification, and staging.
- [ ] 4.4 Add conflict classification, diff review, and explicit installation.
- [ ] 4.5 Verify Mac-to-server and server-to-Mac migration scenarios.

## Phase 5 - Public v0.1 Release

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

- [ ] 5.1 Complete accessibility, dark-mode, and native-window visual QA.
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

# Agent Skill Studio Plan

## Current Position

- Active phase: Phase 5 public v0.1 release
- Current task: 5.2 add CI, release metadata, signing, notarization, and packaging
- Next task: 5.3 reconcile public documentation, screenshots, and limitations
- Last accepted milestone: Task 7.5 multi-Skill repository intake and review
  queue, accepted by the owner on 2026-08-16
- Blocking decision: none; the owner simplified server migration on 2026-08-06
  to direct Codex-assisted installation of a trusted Mac export, with no custom
  headless Bundle CLI or repeated semantic audit
- Validation item: confirm the final minimum macOS version before public release
- Outstanding acceptance: Task 4.6 real Mac-to-Linux migration remains a
  release gate; it has not been marked complete without a real server result
- Human validation: Task 7.5 passed first-user native-window acceptance on
  2026-08-16; Task 5.1 passed its native-window acceptance on 2026-08-08

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
  Automated release implementation completed on 2026-08-17: the repository
  pins Node 22.23.1 and Rust 1.88.0, separates unsigned CI from the protected
  signed candidate workflow, and verifies universal app/DMG metadata plus the
  complete application revision inside the DMG. The locked dependency set
  requires Rust 1.88, which is above the project's Rust-1.85-or-later
  development floor. Real Apple signing, notarization, and installation and
  launch of the exact candidate remain Task 5.2 acceptance gates. Full
  clean-machine workflow acceptance and publication remain Task 5.4 gates.
- [ ] 5.3 Reconcile README, license, product screenshots, and limitations.
- [ ] 5.4 Run clean-machine acceptance and publish the v0.1 artifacts.

## Phase 6 - Skill Package Workspace And Collections

Status: implementation and bilingual desktop workflow accepted by the project
owner on 2026-08-14. The phase plan was accepted on 2026-08-13.

Detailed plan: [06-package-editor-and-collections.md](docs/phases/06-package-editor-and-collections.md)

Outcome: the Studio treats a Skill as a complete package instead of only a
`SKILL.md` document, and lets the owner organize Skills without changing their
installation paths or source ownership.

Tasks:

- [x] 6.1 Add a contained Package module for file tree, preview, validation,
  text editing, file operations, package-level Diff, and direct single-Skill
  export through the existing Bundle format.
- [x] 6.2 Add persistent many-to-many Collections with create, rename, delete,
  filtering, counts, and Skill membership editing.
- [x] 6.3 Record candidate acquisition provenance outside Skill packages and
  group Skills in every catalog and Collection view by exact GitHub repository,
  local source, or unknown provenance without guessing existing history.
- [x] 6.4 Add evidence-aware GitHub update checks that compare the complete
  local and remote package, then route changed content into Candidate Review
  without installing or overwriting it.
- [x] 6.5 Add attributed Git-style text Diff and guarded single-file sync for
  GitHub-added, GitHub-removed, and modified files. Replacing a modified file
  requires an attribution-aware overwrite confirmation and never auto-merges.
  Treat `.DS_Store` at any package depth as Finder metadata: exclude it from
  discovery, revisions, Diff, candidates, Bundle contents, update checks, and
  Deep Audit without requiring the owner to delete it from disk. An imported
  Bundle is fully verified before this metadata is removed from its logical
  review and installation model.
- [x] 6.6 Complete bilingual desktop acceptance for Package, Collection,
  provenance, and update-check workflows, then reconcile release scope.
- [x] 6.7 Give single-Skill export a dedicated save flow instead of the batch
  Bundle page, remove the overlapping existing-Skill editor entry, and move
  baseline Audit, Deep Audit, evidence, and guarded save into the complete
  Package workspace. New-Skill creation remains a separate workflow.

## Phase 7 - Multi-Skill Repository Intake

Status: accepted by the project owner on 2026-08-16 after automated and native
first-user verification. Phase 5 release work has resumed.

Detailed plan: [07-repository-intake-links.md](docs/phases/07-repository-intake-links.md)

Outcome: the existing **Import Skill** workflow can accept an ordinary public
GitHub repository URL, discover its conventional Skills, and let a
non-programmer choose and review candidates without Git or terminal commands.

Tasks:

- [x] 7.1 Revalidate maintained repository-discovery tools and map the existing
  single-Skill intake path before finalizing the bounded metadata strategy.
- [x] 7.2 Extend the existing GitHub intake to discover only the root Skill,
  `skills/*/SKILL.md`, and `skills/*/*/SKILL.md` entries at one immutable public
  GitHub revision, without adding another import entry point or defining a
  private manifest.
- [x] 7.3 Add repository listing and multi-selection to the existing **Import
  Skill** workflow without downloading every Skill package eagerly.
- [x] 7.4 Add a resumable Repository Review Queue with lazy per-Skill staging,
  current-process session reuse, bounded blob downloads, Baseline Audit,
  optional Deep Audit, and separate Installation Confirmation; do not add batch
  installation.
- [x] 7.5 Complete first-user native macOS acceptance for repository listing,
  queue navigation, and separate installation confirmation, then reconcile the
  release sequence.

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

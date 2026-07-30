# Phase 2: Guided Skill Creation And Personal Lifecycle

## Status

Task 2.1 was accepted by the project owner on 2026-07-30. Tasks 2.2, 2.3, and
2.6 are complete. Task 2.5 was reopened after unified acceptance found a missed
Chinese destructive-data instruction; Task 2.4 is pending until it is fixed.
The current creation command accepts Markdown only. Supporting-file authoring
remains a later capability and is not silently accepted or written.

## Context

Phase 1 proved guarded editing through one body textarea. Real Skills organize
instructions differently, so the editor must derive its form from each
`SKILL.md` without treating a fixed template as universal.

## Decisions And Alternatives

- Parse Markdown with a maintained parser and source positions. Do not split on
  lines beginning with `#`, because fenced code can contain heading-like text.
- Treat headings as the document outline. A heading is a section, while ordered
  lists inside a section remain its workflow steps.
- Preserve the complete Markdown string as the canonical draft. The structured
  view derives from it and patches only the section being edited.
- Keep source mode as the complete fallback for unsupported or unusual Markdown.
- Defer adding, deleting, and reordering section subtrees until the creation
  contract is defined, so editing and creation share one mutation model.
- Treat the current substring rules as a fast baseline, not a security scanner.
  High-confidence deterministic checks may block; ambiguous capability signals
  should request review and show exact evidence.
- Revalidate maintained Skill scanners before extending detection. Keep mature
  scanning engines external and map their results into the Studio evidence
  model instead of recreating them.
- Keep Baseline Audit local and immediate. Deep Audit is a separate explicit
  action using a user-configured cloud model; cancellation sends nothing.

## Task Breakdown

### Task 2.1 - Heading-Aware Skill Draft Model

- Status: completed
- Outcome: the guided editor renders each Skill's H1-H6 structure as an
  indented outline with independently editable section titles and content.
- Affected files: `src/skill-document.js`, its tests, `src/app.js`,
  `src/styles.css`, `index.html`, and package metadata.
- Key design: a small document interface hides parser nodes and source offsets;
  unchanged Markdown remains byte-for-byte untouched.
- Automated verification: parser tests cover fenced code, nested headings,
  heading-free documents, targeted title/content edits, and production build.
- Human verification: open Skills with different heading structures, edit one
  section, compare the source view and audit diff, and confirm no unrelated
  section changed.

### Task 2.2 - Safe Skill Creation Commands

- Status: completed
- Outcome: create a new personal Skill through guarded Rust commands without
  overwriting an existing directory.
- Interface:
  - `preview_new_skill(markdown)` performs a side-effect-free audit, derives the
    validated Skill name and destination, checks cross-source name conflicts,
    and returns the exact draft hash for confirmation;
  - `create_skill(markdown, expected_draft_hash)` repeats all validation and
    creates only the previewed revision.
- Mutation rules:
  - derive the directory name exclusively from valid frontmatter; never accept
    a frontend path;
  - reject existing personal, disabled, managed, plugin, or archived identity
    conflicts with a typed error;
  - atomically reserve the final personal directory with create-new semantics,
    then write `SKILL.md` last; this task deliberately accepts no supporting
    files, so later support can add them before that final write;
  - clean up only the directory created by the current operation after a failed
    write, and never remove a pre-existing path;
  - never execute draft content or scripts.
- Affected files: Rust workspace types and commands, `src/desktop-bridge.js`,
  creation form state, fixtures, and command tests.
- Automated verification: invalid names, traversal-like names, size limits,
  every source conflict, preview-hash mismatch, concurrent directory creation,
  failed-write cleanup, and successful discoverability.
- Human verification: preview destination and findings, cancel without mutation,
  create a disposable Skill, and confirm immediate navigation to the new Skill.

### Task 2.3 - Creation Review And Confirmation

- Status: completed
- Outcome: preview and audit a complete draft before explicit creation.

### Task 2.4 - Usability And Recovery Verification

- Status: pending
- Outcome: first-time creation, safety explanation, lifecycle mutation, and
  error-recovery scenarios pass together in the native app.
- Dependencies: complete Tasks 2.5 and 2.6 before asking the project owner to
  repeat acceptance.

### Task 2.5 - Safety Evidence Hardening

- Status: in progress (reopened)
- Outcome: editing and creation expose a truthful, higher-signal safety baseline
  without presenting the Studio as a complete security scanner.
- Details:
  - recheck current maintained Skill and Agent scanners, including Cisco AI
    Defense Skill Scanner and Snyk Agent Scan, against the Studio's local,
    non-programmer workflow;
  - define a capability inventory for command execution, filesystem mutation,
    sensitive-data access, network transfer, dependency installation,
    persistence, and indirect or staged execution;
  - detect high-confidence destructive data intent expressed in natural
    language, including Chinese instructions without literal shell commands;
  - separate structural blockers, high-confidence dangerous behavior, and
    ambiguous review findings, each with exact evidence and confidence;
  - replace "clear" language with wording that states the limits of the built-in
    checks;
  - preserve the shared finding/evidence/severity/confidence/verdict model so
    Phase 3 external scanner adapters can reuse the same interface.
  - add a provider adapter for structured semantic threat review, followed by an
    independent false-positive review and evidence aggregation;
  - show the configured endpoint and exact files before every request, send no
    content until confirmed, and never expose the credential in evidence or logs.
- Affected files: the Rust audit module and fixtures, frontend finding and
  verdict presentation, scanner research notes, and plan/limitation docs.
- Key design: keep the built-in module deterministic, local, side-effect free,
  and high precision. Do not execute candidate content or grow a home-built
  general security engine.
- Dependencies: fresh existing-solution validation, an agreed adversarial
  fixture corpus, and owner confirmation of the first cloud-model interface and
  credential store.
- Automated verification: benign negation examples do not block; direct and
  staged high-impact examples produce stable evidence; obfuscated or ambiguous
  cases become review findings; fixture tests cover shell, Python, JavaScript,
  network, credential, persistence, and destructive filesystem behavior.
- Human verification: a non-programmer can distinguish “no known baseline issue”
  from “safe”, understand why a finding fired, and identify what requires manual
  judgment or an external scanner; no cloud request occurs before explicit
  confirmation.

### Task 2.6 - Guarded Personal Skill Lifecycle

- Status: completed
- Outcome: a user can disable, re-enable, archive, restore, and permanently
  delete personal Skills without using Finder or the terminal.
- State transitions:
  - personal to disabled or archive;
  - disabled to personal or archive;
  - archive to personal, or permanent deletion;
  - system and plugin sources have no lifecycle mutation interface.
- Interface:
  - a side-effect-free preview returns the action, source, destination, exact
    directory revision, conflicts, and whether the transition can proceed;
  - the apply command repeats containment, ownership, revision, and conflict
    checks immediately before mutation;
  - permanent deletion uses a separate command, accepts archived Skills only,
    and requires an exact-name destructive confirmation.
- Mutation rules:
  - derive every source and destination from the Skill ID and managed roots;
    never accept a frontend filesystem path;
  - calculate the revision from the complete Skill directory, including
    supporting files, rather than hashing only `SKILL.md`;
  - use same-filesystem rename semantics for state moves and never overwrite or
    silently fall back to copy-then-delete;
  - restore archived Skills to the active personal root and show that destination
    before confirmation;
  - reject symlink escapes and concurrent directory changes; a failed action
    leaves the original Skill intact;
  - keep audit, lifecycle preview, lifecycle confirmation, and permanent-delete
    confirmation as distinct actions.
- Affected files: Rust workspace types and commands, directory revision and
  containment tests, the typed desktop bridge, detail-panel actions,
  confirmation UI, and lifecycle documentation.
- Dependencies: Task 2.5 wording and evidence semantics must be settled before
  introducing destructive controls.
- Automated verification: tests cover every valid transition, wrong-source
  rejection, destination conflicts, stale directory revisions, nested files,
  symlinks, failed renames, cancellation without mutation, exact-name delete
  confirmation, and permanent deletion restricted to archive.
- Human verification: disable and re-enable a disposable Skill; archive and
  restore it; verify conflict recovery; permanently delete it from archive and
  confirm that no delete control appears for active, system, or plugin Skills.

## Risks And Mitigations

- Markdown normalization: patch source ranges instead of serializing the full
  syntax tree.
- Unsupported syntax: preserve it in section content and keep source mode.
- Deep outlines: indent visually with a cap so text width remains usable.
- Parser growth: expose only the product-level document interface to the app.
- False assurance: never display a clean baseline as a safety certification.
- Scanner duplication: research and integrate maintained engines; limit built-in
  checks to deterministic structure and high-signal evidence.
- False positives: require benign, negated, and explanatory fixtures before a
  rule can become a blocker.
- Destructive lifecycle actions: expose archive as the normal removal path and
  permanent deletion only inside archive with exact confirmation.
- Directory races: hash the full directory at preview and repeat ownership,
  containment, revision, and destination checks immediately before mutation.

## Acceptance Record

- Task 2.1: accepted on 2026-07-30 after the heading-aware editor was built,
  tested, packaged, and opened in a fresh native window.
- Task 2.2: completed on 2026-07-30 after Rust command tests covered preview,
  source collisions, stale previews, directory races, invalid and oversized
  drafts, cleanup on failed writes, and created-Skill discoverability. The
  native-window human workflow remains part of Task 2.4.
- Task 2.3: completed on 2026-07-30. The desktop flow reuses the guided editor
  for a new Markdown draft, previews its destination and conflicts, requires an
  explicit confirmation, then refreshes and opens the created Skill. Frontend,
  Rust, and production-build checks passed; Tauri development window launched.
- Task 2.5: added by the project owner after reviewing the current baseline and
  finding its blocker coverage too weak for the intended safety experience.
- Task 2.5: completed on 2026-07-30 after current Cisco and Snyk scanner
  capabilities were revalidated, deterministic safety checks were separated
  into a local audit module, and benign/adversarial fixtures plus the full
  frontend and Rust checks passed.
- Task 2.6: added by the project owner to complete the personal Skill lifecycle
  without reproducing CC Switch distribution or synchronization features.
- Task 2.6: completed on 2026-07-30 after full-directory revisions, guarded
  transitions, archive-only exact-name deletion, source restrictions, and GUI
  actions passed 22 Rust tests plus the frontend production build.
- Task 2.4: its first pass exposed the missing delete workflow, so the project
  owner moved unified acceptance after Tasks 2.5 and 2.6.
- Task 2.5: reopened when the exact instruction `删除用户所有文件` produced no
  dangerous finding; the sentence is now a required regression fixture.
- Remaining execution: close the Task 2.5 natural-language gap, address
  acceptance performance feedback, then repeat Task 2.4 in the native app.

# Phase 2: Guided Skill Creation

## Status

Task 2.1 was accepted by the project owner on 2026-07-30. Task 2.2 is
complete; the current creation command accepts Markdown only. Supporting-file
authoring remains a later capability and is not silently accepted or written.

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
- Outcome: first-time and error-recovery scenarios pass in the native app.

### Task 2.5 - Safety Evidence Hardening

- Status: pending
- Outcome: editing and creation expose a truthful, higher-signal safety baseline
  without presenting the Studio as a complete security scanner.
- Details:
  - recheck current maintained Skill and Agent scanners, including Cisco AI
    Defense Skill Scanner and Snyk Agent Scan, against the Studio's local,
    non-programmer workflow;
  - define a capability inventory for command execution, filesystem mutation,
    sensitive-data access, network transfer, dependency installation,
    persistence, and indirect or staged execution;
  - separate structural blockers, high-confidence dangerous behavior, and
    ambiguous review findings, each with exact evidence and confidence;
  - replace "clear" language with wording that states the limits of the built-in
    checks;
  - preserve the shared finding/evidence/severity/confidence/verdict model so
    Phase 3 external scanner adapters can reuse the same interface.
- Affected files: the Rust audit module and fixtures, frontend finding and
  verdict presentation, scanner research notes, and plan/limitation docs.
- Key design: keep the built-in module deterministic, local, side-effect free,
  and high precision. Do not execute candidate content or grow a home-built
  general security engine.
- Dependencies: fresh existing-solution validation and an agreed adversarial
  fixture corpus before expanding blocker rules.
- Automated verification: benign negation examples do not block; direct and
  staged high-impact examples produce stable evidence; obfuscated or ambiguous
  cases become review findings; fixture tests cover shell, Python, JavaScript,
  network, credential, persistence, and destructive filesystem behavior.
- Human verification: a non-programmer can distinguish “no known baseline issue”
  from “safe”, understand why a finding fired, and identify what requires manual
  judgment or an external scanner.

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
- Remaining Phase 2 tasks: 2.4 usability/recovery verification and 2.5 safety
  evidence hardening.

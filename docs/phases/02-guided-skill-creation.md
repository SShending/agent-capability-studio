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

- Status: pending
- Outcome: preview and audit a complete draft before explicit creation.

### Task 2.4 - Usability And Recovery Verification

- Status: pending
- Outcome: first-time and error-recovery scenarios pass in the native app.

## Risks And Mitigations

- Markdown normalization: patch source ranges instead of serializing the full
  syntax tree.
- Unsupported syntax: preserve it in section content and keep source mode.
- Deep outlines: indent visually with a cap so text width remains usable.
- Parser growth: expose only the product-level document interface to the app.

## Acceptance Record

- Task 2.1: accepted on 2026-07-30 after the heading-aware editor was built,
  tested, packaged, and opened in a fresh native window.
- Task 2.2: completed on 2026-07-30 after Rust command tests covered preview,
  source collisions, stale previews, directory races, invalid and oversized
  drafts, cleanup on failed writes, and created-Skill discoverability. The
  native-window workflow remains part of Task 2.3 and Task 2.4.
- Remaining Phase 2 tasks: 2.3 and 2.4.

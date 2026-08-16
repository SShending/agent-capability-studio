# Phase 7: Multi-Skill Repository Intake

## Status

Accepted by the project owner on 2026-08-16 after implementation, automated
verification, and native macOS first-user acceptance. The public v0.1 release
sequence has resumed.

## Outcome

The existing **Import Skill** workflow accepts the same ordinary public GitHub
URL it accepts today. When that URL identifies a repository containing multiple
conventional Skills, the Studio lists them and creates a review queue from the
user's selection instead of requiring Git, Node.js, file copying, directory
creation, or terminal commands.

## Confirmed boundaries

- Phase 7 extends the existing **Import Skill** action and GitHub-address field.
  It does not add another top-level import button or a duplicate paste workflow.
- Submitting a repository source does not imply Audit or installation. The app
  shows the public GitHub source for confirmation before repository discovery.
- A source may name an optional branch, tag, or commit. After confirmation, the
  Studio resolves it, or the repository's default branch when omitted, to one
  immutable commit SHA. The listing and queue remain bound to that SHA until an
  explicit refresh or new intake.
- Repository Intake resolves one immutable revision and discovers only a root
  `SKILL.md`, `skills/*/SKILL.md`, and `skills/*/*/SKILL.md` entries. The second
  bounded form covers repositories that group Skills by category without
  turning discovery into arbitrary recursion.
- When both forms exist, all are separate candidates. The root candidate appears
  first and is clearly labelled as the repository-root Skill; the Studio does
  not infer that it represents or owns the nested Skills.
- A URL that already points to one Skill directory remains a single-candidate
  workflow.
- The Studio does not recursively treat every `SKILL.md` as installable and does
  not define a private repository manifest format in this phase.
- The user may select multiple discovered Skills. Selection creates a Repository
  Review Queue whose entries retain separate staging, Baseline Audit, comparison,
  and Installation Confirmation state.
- The initial Repository Candidate Listing contains revision-bound directory
  names and paths from GitHub metadata only. It does not download every
  `SKILL.md` merely to show descriptions.
- Before presenting the listing, the Studio removes candidates whose repository
  and exact Skill path match confirmed or recorded provenance in the current
  local catalog. Disabled and archived matches still count as installed. A name
  or directory match without source evidence remains visible to avoid hiding a
  different Skill.
- Review Queue entries stage lazily when the user opens them. Only then does the
  Studio download that complete Candidate Skill and run its local Baseline
  Audit.
- A multi-item Candidate Review provides previous and next controls beside the
  queue position. The first visit lazily stages that entry at the immutable
  revision; later visits in the same queue reuse its verified in-memory session
  without another GitHub download. Starting a new intake, installing or removing
  an entry, and exiting the app discard the affected ephemeral sessions.
- Repository Intake Sessions persist only the public source, immutable revision,
  selected paths, and current queue position. Staged files, Diff, and Audit
  Results remain ephemeral; after restart, uninstalled entries must be staged
  and reviewed again.
- There is no repository-level or “install all clear” confirmation. Identical,
  conflicting, blocked, and new candidates remain distinct outcomes.
- Deep Audit remains optional and requires its existing per-scan provider and
  exact-file consent. Repository Intake never starts cloud review automatically.
- One listing contains at most 256 discoverable Skills. The limit applies before
  presenting a partial result; staging limits continue to apply independently to
  each selected Skill.

## Existing-solution boundary

CC Switch and Vercel Skills already own broad repository distribution, updates,
and multi-Agent installation. This phase is defensible only as a no-terminal
handoff into the Studio's evidence and guarded-mutation workflow. It must not
grow into registry discovery, synchronization, or a competing bulk installer.

## Implementation design

- Keep GitHub URL parsing, ref resolution, metadata discovery, containment, and
  fixed-revision staging inside the existing Rust candidate module. Its typed
  desktop interface gains one metadata-listing command and one command that
  stages an allowed Skill path at the listing's immutable commit.
- Cache the listing's recursive tree only for the current app process and key it
  by repository, requested ref, and immutable commit SHA. Reuse it to avoid
  repeated commit and path traversal requests, but never treat it as installation
  authority. Download one selected Skill's blobs with at most six workers,
  validate every declared size, and preserve deterministic manifest ordering.
- Keep queue presentation and resumable queue metadata in a small frontend state
  module. Persist only the public source, requested ref, resolved SHA, selected
  paths, and current position. Never persist staged bytes, candidate session
  identifiers, Diff, or Audit Results.
- Extend the existing Import Skill dialog through source, repository-listing,
  and review-queue states. Do not add another navigation entry or paste field.
- Opening a queue entry delegates to the existing Candidate Stager, Baseline
  Audit, Deep Audit, installation preview, and installation commands. Closing an
  entry hides its temporary review while preserving it in the current in-memory
  queue; a new intake drains all retained staging sessions.
- Installing one entry removes only that entry from the queue. Starting another
  repository intake explicitly replaces the previous queue metadata.

## Task breakdown

### 7.1 - Existing-solution and code-path validation

Complete. Broad repository distribution remains owned by maintained tools; the
Studio's difference is review and guarded mutation through its existing intake.

### 7.2 - Immutable repository discovery and lazy staging

- Return repository, requested ref, resolved SHA, and conventional Skill paths
  without downloading Skill content.
- Reuse the same GitHub transport and input validation as single-Skill staging.
- Stage a selected path by resolved SHA so a moving branch cannot change the
  reviewed revision.

### 7.3 - Listing and selection

- Add bilingual repository listing and selection to the existing dialog.
- Create and validate the minimal queue metadata without downloading selected
  Skill packages.

### 7.4 - Review queue

- Persist and validate the minimal queue metadata locally.
- Open and discard one staged candidate at a time; keep installation confirmation
  independent for every item.

### 7.5 - Acceptance and reconciliation

- Run focused Rust and frontend tests, the full automated suite, native macOS
  workflow validation, and bilingual visual/accessibility review.
- Reconcile `PLAN.md` and user-facing documentation after acceptance.

## Automated verification record

- A live metadata check of `mattpocock/skills` on 2026-08-16 found 35 Skills at
  `skills/<category>/<skill>/SKILL.md`; this evidence produced the bounded second
  directory level instead of arbitrary recursive discovery.
- 49 frontend tests passed, including installed-provenance filtering, queue
  metadata validation, explicit-ref
  restoration, in-memory session reuse and cleanup, position changes, and
  bilingual catalog parity.
- 163 desktop-core tests passed with one owner-catalog timing benchmark ignored
  by design. They include deterministic metadata and blob request counts,
  fixed-SHA lazy staging, bounded concurrent downloads, failure cleanup,
  deterministic manifest ordering, root/nested separation, truncation,
  collision, empty-result, and 256-item limit behavior.
- 24 portable Bundle-core tests passed, and the production WebView build
  completed. The owner accepted repository listing, queue navigation, staging
  reuse, and separate installation confirmation in the native window on
  2026-08-16.

## Verification matrix

- Discovery: root, direct Skill children, and one category level are listed;
  deeper/example paths are ignored, root appears first, and zero or more than
  256 candidates is rejected.
- Network: listing downloads zero blobs; opening one queue item downloads only
  that item's files and remains bound to the listed SHA.
- Safety: malformed or credential-bearing URLs, traversal, unsafe entries,
  truncated trees, case collisions, and per-candidate resource excess fail
  before installation and never execute repository content.
- Queue: selection is a subset of the listing, switching away and back reuses
  current-process staging, restart restores metadata only, a new intake drains
  retained sessions, and installing one item never installs another.
- UX: the one/many/local paths remain understandable in Chinese and English;
  keyboard navigation, narrow layouts, and a real macOS window are verified.

## Non-goals

- Batch installation or one confirmation covering multiple Skills.
- Automatic Baseline or Deep Audit of unselected repository content.
- Private repositories, GitHub credentials, arbitrary Git hosts, submodules, or
  symlink preservation without separate product and threat-model decisions.
- A custom URL scheme, browser extension, repository badge, or other external
  handoff surface without a validated place that can publish or invoke it.
- Renaming the desktop product before a second capability type is implemented
  and accepted.

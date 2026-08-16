# Phase 6 - Skill Package Workspace And Collections

Status: completed and accepted by the project owner on 2026-08-14.

## Scope

### Complete Skill Package

- Display the complete Skill directory as a file tree, including `references/`,
  `scripts/`, `assets/`, `agents/`, and other metadata files.
- Preview UTF-8 text and supported images without executing package content.
- Let the owner create, edit, rename, and delete contained files and folders in
  a personal active Skill. `SKILL.md` remains required and cannot be deleted or
  renamed.
- Validate path containment, symlinks, resource limits, required files, text
  encoding, and executable/support-file risks.
- Compute file-level and package-level changes against the state captured when
  the Package workspace opened. Save with a complete-directory revision check.
- Treat a path component named exactly `.DS_Store` as Finder metadata rather
  than Skill content. It may remain on disk, but discovery, file counts,
  revisions, hashes, Package trees, Diff, candidate fingerprints, update
  checks, Bundle export, install comparison, and Deep Audit exclude it. Bundle
  import first verifies the complete archive and manifest, including any
  declared `.DS_Store`, then excludes it from the logical review and install.
- Reuse the existing verified Skill Bundle format and verification core for
  whole-package import/export. The selected Skill's Export action uses a
  dedicated save flow that cannot select or export any other Skill; batch export
  remains a separate Library workflow. Import remains a separate staged
  installation workflow.
- Use the complete Package workspace as the only editing surface for an existing
  Skill. It owns `SKILL.md`, supporting-file edits, baseline Audit, optional Deep
  Audit, evidence, Package Diff, and guarded save. New-Skill creation remains a
  separate workflow because it has no existing Package identity or directory.

### Collections

- Keep source (`personal`, `system`, `plugin`, `disabled`, `archive`) as factual
  installation ownership and lifecycle state.
- Store Collections as Studio-owned metadata. They never move a Skill directory
  and never alter what Codex loads.
- A Skill may belong to zero, one, or many Collections.
- Create, rename, delete, filter, and edit membership. Deleting a Collection
  removes only its metadata.
- Bind membership to stable source plus canonical directory identity, and prune
  missing members when reading without mutating Skill contents.

### Acquisition provenance

- Keep Management Source (`personal`, `system`, `plugin`, `disabled`,
  `archive`) separate from Acquisition Provenance.
- Persist the exact GitHub repository, requested ref, resolved commit, and
  repository-relative Skill path after a Studio candidate installation.
- Record a Studio candidate installed from a local directory as local without
  exposing its absolute path as a list heading.
- Classify pre-existing and otherwise unrecorded Skills as Unknown Provenance.
  Trusted installation history or explicit package identity metadata may confirm
  a legacy repository without inventing an original commit; names, content
  similarity, and incidental links are not sufficient evidence.
- Keep provenance in Studio-owned application metadata, outside Skill packages
  and Bundle v1 exports, and retain it across lifecycle moves.
- Show the same GitHub repository, local, and unknown provenance grouping in
  every catalog view, including Collections and lifecycle-state filters. Keep
  the per-row management-source badge separate from acquisition provenance.

### GitHub update checks

- Offer Check for Updates only for user-controlled Skills with confirmed or
  recorded GitHub provenance and a repository-relative Skill path.
- Resolve and stage the current remote package through the existing GitHub
  candidate acquisition module, then compare its complete file fingerprint with
  the current local package. Checking is read-only and never replaces files.
- Report `up to date` only when the complete remote and local package
  fingerprints match. When an exact installed commit was recorded, distinguish
  a newer remote commit from local edits at the same commit. For legacy
  repository confirmations without an installed commit, report only that the
  contents differ; do not claim that either side is newer.
- Let a changed remote package enter the existing Candidate Review, baseline
  Audit, optional Deep Audit, and file preview workflow. Close or discard removes
  the staged copy. Updating the installed Skill remains a separately designed,
  explicitly confirmed mutation and is not part of this task.
- Present changed text with a bounded Git-style line comparison. Red and green
  describe content removed from the local version and content added by the
  GitHub version; they do not claim which side originally authored the change.
  Use a separate blue Local label, orange Remote label, or Unknown label for
  attribution. Attribute a change only when the recorded install fingerprint
  and commit support that conclusion. Legacy repository confirmations without a
  retained content baseline must say that attribution is unknown.
- Allow explicitly confirmed single-file sync for files newly added on GitHub,
  removed on GitHub, or modified. Modified-file sync replaces the complete local
  file with the GitHub version and never claims to merge it. Use a destructive
  confirmation when local edits are known or attribution is unknown. Recheck
  the staged candidate hash, current local package revision, path containment,
  and action-specific add, replace, or delete precondition immediately before
  committing through the Package module's staged atomic replacement.

## Architecture

The Rust `SkillPackage` module is a deep module with a narrow command interface.
It hides recursive traversal, containment, file typing, revision calculation,
atomic writes, and post-save catalog refresh. The frontend receives a snapshot,
submits a complete text mutation set, and renders the returned result.

The Rust `Collections` module owns a versioned JSON document in the Tauri app
configuration directory. It validates names and identifiers, writes atomically,
and exposes collection snapshots plus membership mutations. Collection metadata
is not placed in Skill directories or exported in v1 Skill Bundles.

The Rust `Provenance` module owns a separate versioned, private JSON document in
the same application configuration directory. Its narrow interface records a
confirmed candidate installation, attaches known provenance to catalog results,
and migrates or removes records after lifecycle actions.

## Acceptance

- A personal Skill can edit `SKILL.md` and supporting text files in one Package
  workspace, see unsaved changes, and save without overwriting external changes.
- Unsafe paths, symlinks, oversized packages, binary text edits, removal of
  `SKILL.md`, and read-only sources are rejected.
- Images preview locally; scripts are shown as text and never run.
- Exporting the current personal Skill opens a dedicated save flow and exports
  exactly that Skill through the verified Bundle format. It never opens the
  batch selection page. Imported bundles still require staged review and
  install.
- Existing personal Skills expose one Package editor rather than separate
  `SKILL.md` and Package editors. Baseline Audit, Deep Audit consent and evidence,
  complete Package changes, and save confirmation all refer to the same pending
  Package revision.
- A Collection can be created, renamed, deleted, selected, and assigned to any
  discovered Skill. One Skill can appear in multiple Collections.
- Restarting the app preserves Collections and membership. Deleting a Collection
  never deletes or moves a Skill.
- Personal Skills are grouped by exact GitHub repository, Local Source, or
  Unknown Provenance. Legacy Skills remain unknown unless trusted installation
  history or explicit package identity confirms a repository; the original
  revision remains absent unless Studio observed it during installation.
- GitHub provenance survives disable, archive, restore, and application restart;
  permanent deletion removes its active identity record.
- A user-controlled Skill with GitHub provenance can check the current remote
  package. Identical content is reported without opening Candidate Review;
  changed or incomparable legacy content can be opened in Candidate Review and
  cannot silently overwrite the installed Skill.
- Update review colors and labels distinguish evidence-backed local edits from
  remote changes and explicitly identify unknown attribution. Remote-only added
  and removed files can be synchronized one at a time after confirmation.
  Modified files can be replaced with the complete GitHub version only after an
  attribution-aware overwrite confirmation; the Studio never presents this as
  an automatic merge. When the complete local package then matches the staged
  candidate, provenance advances to that exact commit.
- Adding or changing `.DS_Store` at any Package depth does not change the
  displayed file count, package revision, update status, Bundle payload, or
  Deep Audit selection. Imported copies are integrity-checked but never
  installed.
- Simplified Chinese and English common paths are complete, tests pass, and the
  owner validates the native macOS workflow.

## Performance evidence

- The deterministic 121-Skill catalog test covers Package open, preview, and
  save with one full catalog scan and seven affected-Package revision reads.
- On 2026-08-14, the ignored read-only owner-catalog benchmark indexed 60
  installed Skills in 851 ms, opened a selected three-file Package in 14 ms,
  and recorded one full catalog scan plus one Package revision read.
- Reproduce the owner benchmark with
  `cargo test --manifest-path src-tauri/Cargo.toml owner_catalog_package_open_timing -- --ignored --nocapture`.

## Code review remediation record

On 2026-08-14, the Phase 6 working tree was reviewed against both repository
Standards and this specification. Duplicate Standards and Spec findings were
consolidated before implementation. The confirmed issues were resolved as
follows:

- Lifecycle moves and permanent deletion now coordinate filesystem mutation
  with Collection and provenance metadata finalization. A failed metadata
  update rolls the directory transition back, and deletion repeats its boundary
  and revision checks before destructive removal.
- Package, Candidate sync, Bundle installation, and lifecycle recovery reuse a
  contained directory-replacement boundary. Failures preserve or explicitly
  identify recoverable content instead of silently leaving filesystem and
  Studio metadata in different states.
- Candidate single-file sync is bound to the complete staged snapshot, current
  Package revision, path containment, and action-specific precondition. Exact
  GitHub synchronization restores the prior Package if provenance cannot be
  advanced to the synchronized commit.
- Bundle installation performs a strict fresh cross-source conflict check at
  apply time, preserves successful per-Skill receipts if a later item fails,
  and returns enough detail for an incremental frontend catalog update.
- Asynchronous Bundle export and import previews are bound to the active dialog,
  session, and revision so a stale response cannot publish into a newer
  workflow.
- Simplified Chinese fallback text, native file-dialog labels, generated
  starter content, and the localization catalog use consistent domain terms:
  技能, 技能包, 技能迁移包, and 分组. Proper names and technical identifiers
  such as Codex, GitHub, `SKILL.md`, and SHA-256 remain unchanged.

The previously closed `.DS_Store` requirement was not reopened as a new review
finding. Its existing specification and regression coverage remain in force.

Final verification completed with 35 frontend tests, 153 desktop Rust tests,
24 Skill Bundle core tests, the production Web build, Rust formatting, Clippy
with warnings denied, and `git diff --check`. The original-scope Standards and
Spec re-review reported no remaining confirmed findings. The project owner then
accepted the Simplified Chinese and English desktop workflows on 2026-08-14.

## Task 6.7 acceptance record

On 2026-08-15, the owner accepted the workflow consolidation follow-up:

- Exporting from a selected Skill now opens a dedicated native save flow and
  exports exactly that Skill through the verified Bundle backend. It does not
  open or reuse the batch export page.
- Existing Skills now have one Package workspace. The overlapping `SKILL.md`
  editor entry and its existing-Skill save path were removed; new-Skill creation
  remains separate.
- Baseline Audit, Deep Audit consent and evidence, Package validation, text
  Diff, and guarded save now operate on the same pending Package revision.
  Deep Audit sees unsaved supporting files and rechecks the source revision,
  proposed revision, candidate hash, provider hash, and exact file selection
  before sending.

Verification completed with 37 frontend tests, 155 desktop Rust tests, 24 Skill
Bundle core tests, the production Web build, Rust formatting, Clippy with
warnings denied, and `git diff --check`.

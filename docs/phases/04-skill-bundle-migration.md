# Phase 4: Skill Bundle Migration

## Context

The first user wants to move personal Codex Skills from the local Mac to a
Linux server. Phase 4 adds a portable artifact and a guarded desktop workflow;
for the owner's trusted self-export, Codex on the server performs the final
installation through existing shell and transfer tools.

Existing-format validation is recorded in
[skill-bundle-formats.md](../research/skill-bundle-formats.md). No inspected
maintained format supplies the exact multi-Skill manifest, staged verification,
conflict comparison, and separate Installation Confirmation required here.

## Scope

- Preview and export selected eligible user-controlled Codex Skills. The exact
  v1 lifecycle states remain an owner decision below.
- Write a versioned, hash-verified Skill Bundle without overwriting a file.
- Verify an imported bundle inside app-owned temporary staging.
- Present exact Skills, files, revisions, compatibility, audit evidence, and
  conflicts before any target mutation.
- Classify each imported Skill as new, identical, user conflict, managed
  conflict, or incompatible.
- Install only explicitly selected revisions after a separate final recheck.
- Verify the owner's real Mac-to-Linux-server workflow with Codex performing a
  direct trusted installation. Do not repeat semantic audit or add a custom
  headless Bundle CLI unless this real workflow proves one is necessary.

## Non-Goals

- Cloud synchronization, SSH transfer, WebDAV, S3, registry publication, update
  discovery, or automatic distribution.
- Exporting system or plugin-managed Skills.
- Treating lifecycle history or target state as backup data in v1.
- Preserving ownership, ACLs, extended attributes, timestamps, or permission
  bits other than executable state.
- Bundle encryption, signatures, authenticity claims, or trust stores in v1.
- Installing during Bundle Import or executing any bundled content.
- Claiming compatibility with Agents other than the tested Codex contract.

## Decisions And Alternatives

### User workflow

The v1 default and eligibility are active personal Exportable Skills only.
Disabled and Archived Skills remain lifecycle states rather than transport or
automatically restored target state. This keeps the artifact focused on moving
the owner's current Codex capabilities rather than recreating backup and
synchronization.

Export uses preview and apply as separate actions. Apply rechecks ownership,
containment, secrets, and the complete source revision before writing. Bundle
Import verifies and stages only; installation remains a later explicit action.

### Deep module

Use a pure Rust `skill-bundle-core` crate with no Tauri, WebView, Node.js, or GUI
dependency. The macOS `SkillBundleManager` owns desktop export/import behavior;
the pure core keeps format semantics deterministic without creating a second
CLI product.

The desktop manager's eventual caller-facing Interface contains these actions:

```rust
preview_export(workspace, skill_ids) -> BundleExportPlan
export(workspace, expected_plan_revision, destination) -> BundleExportReceipt
stage_import(workspace, source) -> BundleImportReview
discard_import(session_id) -> ()
```

A crate-private verified-snapshot method supplies exact staged bytes to the
comparison and installation implementation. The frontend never supplies
archive entries, hashes, staging paths, placement paths, or arbitrary Agent
configuration.

ZIP encoding is a concrete implementation dependency, not a public archive
Adapter. Add another format seam only if a second maintained format is actually
supported.

### Bundle v1

Suggested extension: `.skillbundle`. Container: ZIP with UTF-8 entry names and
only Stored or Deflate regular-file entries.

```text
skill-bundle.json
skills/<directory-name>/SKILL.md
skills/<directory-name>/<supporting-path>
```

Directory entries, links, devices, encrypted files, split archives, unsupported
compression, duplicate names, and unmanifested files are invalid.

The physical ZIP layout is canonical: `skill-bundle.json` is the first local
record, followed by Skill files in the same ascending bytewise order as the
manifest. Central-directory records use that identical order. Local records
are contiguous from byte zero through the start of the central directory; the
central directory and end record are contiguous through end-of-file. Entry
comments, archive comments, extra fields (including Info-ZIP Unicode Path),
data descriptors, prefixes, trailing bytes, and Zip64 are unsupported in v1.

The root manifest is strict UTF-8 JSON:

```json
{
  "format": "agent-skill-studio/skill-bundle",
  "formatVersion": 1,
  "agentContract": {
    "id": "codex",
    "version": 1
  },
  "skills": [
    {
      "directoryName": "example-skill",
      "revision": "<64 lowercase hexadecimal SHA-256>",
      "files": [
        {
          "path": "SKILL.md",
          "size": 421,
          "sha256": "<64 lowercase hexadecimal SHA-256>",
          "executable": false
        }
      ]
    }
  ]
}
```

Skills and file records use ascending bytewise order of their validated UTF-8
`directoryName` or relative `path`. Unknown v1 fields,
duplicate JSON keys, unsupported versions, and duplicate or portability-
colliding paths are rejected.

Revision hashing uses SHA-256 with explicit binary framing; it never hashes
ad-hoc joined text or serializer-dependent JSON:

- Integers are unsigned big-endian (`u32` for counts, lengths, and versions;
  `u64` for file sizes).
- Strings are `u32 byte length || exact validated UTF-8 bytes`.
- A Skill revision hashes `"ASS-SKILL\0" || u32(1) || u32(file_count)` followed
  by each bytewise-path-sorted file record: framed path, `u64(size)`, 32 decoded
  SHA-256 bytes, then one executable byte (`0` or `1`).
- A Bundle revision hashes `"ASS-BUNDLE\0" || u32(formatVersion)`, the framed
  Agent contract ID, `u32(contractVersion)`, and `u32(skill_count)`, followed by
  each bytewise-directory-sorted Skill record: framed `directoryName` and the
  32 decoded Skill revision bytes.

The parser requires lowercase 64-character hexadecimal digests and recomputes
both revision levels. These algorithms identify every meaningful v1 field
without embedding a self-hash or depending on ZIP metadata.

The manifest contains no source absolute path, username, hostname, Target
Scope, credential, provider setting, Audit Result, log, lifecycle history, or
installation destination. SHA-256 establishes integrity, not authorship or
security.

### Path and mode policy

- Require relative UTF-8 paths with `/` separators and normal components.
- Reject absolute paths, drive prefixes, backslashes, NUL/control characters,
  Windows-forbidden characters and reserved device names, trailing dots/spaces,
  empty, `.` or `..` components, and excessive component/path/depth lengths.
- Reject duplicate paths and collisions using this frozen v1 portability key:
  Unicode 17.0 canonical decomposition (NFD), Unicode 16.0 Default Case
  Folding in full/default mode, then Unicode 17.0 NFD again. This intentionally
  uses `caseless` 0.2.2 and `unicode-normalization` 0.1.25 data; changing either
  Unicode data version requires a Bundle format version change.
- Require each `directoryName` to be one non-empty normal path component. Reject
  `/`, `\\`, `.`, `..`, control characters, excessive length, and duplicate or
  frozen-v1-portability-key collisions across Skills.
- Require exactly one root `SKILL.md` per Skill.
- Treat the manifest's executable boolean as the portable mode. Ignore archive
  ownership and ordinary permission bits after rejecting non-regular entries.
- Stage all imported files non-executable; apply `0644` or `0755` only during a
  separately confirmed installation.

### Resource policy

Initial application limits:

- 256 Skills per bundle.
- 512 files and 64 MiB per Skill.
- 8,192 files total.
- 16 MiB per file.
- 256 MiB compressed input and 512 MiB total uncompressed Skill payload,
  excluding the separately capped manifest.
- 1 MiB manifest, 16 path levels, 255 UTF-8 bytes per component, and 1,024
  UTF-8 bytes per relative path.

Parsing and hashing must stream; declared ZIP sizes are not trusted. The first
user's current catalog is about 15 MiB across 322 files with no file over 2 MiB,
so these limits provide headroom without making resource exhaustion unbounded.

### Conflict evidence

An imported Skill retains the complete list of same-name matches from personal,
disabled, archive, system, and plugin sources. Its primary Import Classification
uses this precedence:

1. unsupported Agent contract or incompatible document;
2. any exact complete-directory revision in any source: identical and skip;
3. any divergent system or plugin match: managed conflict;
4. any divergent user-controlled match: user conflict;
5. no match: new.

A lower-priority match is never discarded from the review. An identical match
prevents duplicate installation even if another divergent match also exists;
without an identical match, a managed conflict always prevents replacement.
Installation repeats this classification against current files so importing or
installing the same Bundle twice remains a no-op rather than an overwrite.

## Accepted Decisions

- The first server target is Linux and normally uses Codex CLI. For the owner's
  trusted Mac export, Codex installs the Skill directories directly; no second
  semantic audit or custom Bundle CLI is required.
- v1 exports active personal Skills only and automatically skips exact revisions
  already present on import or at final installation.
- The v1 boundary remains a custom strict ZIP, exact revision algorithms,
  executable-only mode preservation, no signatures/encryption, out-of-product
  transfer, and the initial resource limits above.

## Task Breakdown

### Task 4.1 - Format Specification And Fixtures

- Status: completed on 2026-08-05 after owner acceptance on 2026-08-04.
- Outcome: the v1 schema, canonical revisions, errors, and archive-validation
  rules are executable through Rust fixture tests.
- Affected files: a new pure Rust crate under `src-tauri/crates/skill-bundle-core/`,
  the Rust workspace manifests, and deterministic valid/adversarial fixture
  builders inside that crate.
- Implementation: use a maintained Rust ZIP codec; do not implement compression
  manually. Keep ZIP types private. Parse duplicate-aware strict JSON and return
  typed manifest evidence or stable safe errors. Validate the end record,
  central directory, each local header, matching UTF-8 name/flags/method/CRC and
  sizes, and non-overlapping bounded data regions. Reject data descriptors,
  archive prefixes/trailing bytes, multi-disk and ZIP64 records, comments,
  alternate Unicode path fields, and unsupported extra fields before extraction.
- Automated verification: valid minimal, multiple-Skill, nested binary, and
  executable fixtures; wrong version/format, duplicate key/name/path, traversal,
  absolute/Windows/backslash paths, portability-key collision,
  symlink/special/encrypted
  entry, missing/extra file, size/hash/revision mismatch, unsupported
  compression, malformed/truncated ZIP, local/central mismatch, overlapping
  regions, prefix/trailing bytes, data descriptors, Zip64, alternate
  Unicode name, extra-field ambiguity, CRC failure, and resource-limit cases.
- Human verification: inspect the documented v1 manifest and confirm that the
  migration boundary and default export eligibility match the intended product.

### Task 4.2 - Safe Export

- Status: completed and accepted by the project owner on 2026-08-05.
- Outcome: the user previews eligible Skills and writes one new Bundle without
  changing any Skill or overwriting an existing file.
- Key design: reject a selected Skill as a whole on unsafe entries or likely
  secret material; never silently omit files. Recheck the complete directory
  revision immediately before deterministic archive creation and no-replace
  commit. Walk from an anchored Skill-directory handle, open every path without
  following any component link, require a regular file, bind its file identity,
  and stream both hashing and archive bytes from that same handle. Recheck the
  opened identity and complete directory revision after reading.
- Automated verification: ownership, symlink and special-file rejection,
  credential-path and private-key fixtures, source changes after preview,
  link/file replacement during discovery and reading, destination races,
  executable preservation, deterministic revisions, cleanup, and no
  process/network invocation.
- Secret boundary: block known credential filenames and directories, private-key
  blocks, and bounded high-confidence credential assignment/token patterns.
  Findings expose only a rule ID and relative path, never matching bytes. State
  plainly that deterministic checks cannot prove arbitrary secrets are absent
  and that v1 Bundles are unencrypted.
- Implemented interface: the GUI sends only selected catalog IDs to preview and
  an opaque backend `planRevision` plus a user-picked destination to apply. The
  backend keeps plan membership, source revisions, file handles, manifest
  construction, and no-replace authority out of the WebView. Canonical Stored
  ZIP output is preflighted against its actual archive-size ceiling and is
  inspected again through the committed file handle before success is reported.
  On Unix, temporary creation and no-replace commit are relative to one anchored
  destination-directory handle; replacing the visible parent path cannot
  redirect the write. While commit is in flight, the GUI binds the response to
  one operation identity and prevents close, cancel, and Escape from detaching
  the persistent receipt.
- Human verification: export disposable personal Skills, review blocked reasons,
  choose a Finder destination, and inspect the receipt.

### Task 4.3 - Contained Import And Review

- Status: completed and accepted by the project owner on 2026-08-05.
- Outcome: selecting a Bundle creates a temporary verified import session and a
  review, without writing any Agent scope.
- Key design: validate all entries before contained streaming extraction; stage
  non-executable files; run Codex compatibility and Baseline Audit on verified
  bytes; clean failed, cancelled, closed, and abandoned sessions.
- Implemented interface: the selected source is copied from one no-follow file
  handle into an opaque app-cache session. `skill-bundle-core` validates the
  whole archive, then rechecks size and hash while each declared file is passed
  to a directory-handle-relative staging writer. File preview repeats contained
  no-follow reads and verifies the complete hash while returning at most 512 KiB
  of UTF-8 text to the WebView. Import compatibility requires a readable
  frontmatter document, a non-empty description, and matching manifest-directory
  and document identities. Compatibility and Baseline Audit are displayed as
  separate evidence, including severity, confidence, file/line, and exact
  finding evidence. Import review exposes no installation action and discard
  removes only the registered session. File-preview responses are bound to the
  current session ID and Bundle revision, so a discarded session cannot render
  into a later review.
- Automated verification: every invalid fixture leaves no outside-staging write;
  session/revision tampering is rejected; second-pass decompression cannot emit
  beyond a manifest file size; oversized and binary preview behavior is bounded;
  startup cleanup is contained; bundled scripts never execute.
- Human verification: open a valid and invalid Bundle, inspect exact files and
  evidence, cancel, and confirm the installed catalog is unchanged.

### Task 4.4 - Conflict Comparison And Installation Confirmation

- Status: accepted by the owner on 2026-08-06.
- Outcome: each imported Skill is classified and compared, then only explicit
  decisions are installed after final targeted rechecks.
- Key design: identical revisions skip; new Skills use no-replace installation;
  divergent user-controlled conflicts require comparison and an explicit
  decision; system/plugin conflicts are blocked. Installation rechecks the
  staged revision, target ownership, current conflicts, and affected directory
  revisions under the Workspace mutation lock.
- Automated verification: new/identical/user/managed/incompatible classification,
  late conflicts, stale target and import revisions, no managed mutation,
  failure recovery, and targeted index updates without a full refresh.
- Human verification: import an unchanged self-export and observe an identical
  no-op; then compare and replace one disposable personal conflict and confirm
  the named per-Skill receipt. Managed/incompatible gates and late-conflict
  recovery remain automated because the owner should not manufacture managed
  filesystem conflicts by hand.

### Task 4.5 - Codex-Assisted Trusted Server Installation

- Status: completed and accepted by the owner on 2026-08-06; the real Linux
  migration is the separate Task 4.6 acceptance pass.
- Outcome: the owner transfers a trusted Mac export using an existing tool and
  asks Codex on the Linux server to install its Skill directories directly.
- Key design: do not repeat semantic audit. Extract into a temporary directory,
  copy missing Skills, skip identical content, and ask before replacing a
  different same-name server Skill. Never silently overwrite server changes.
- Automated verification: the exported artifact remains a conventional ZIP and
  deterministic fixtures document its paths and manifest.
- Human verification: transfer one real Bundle, ask Codex to install it, and
  confirm the expected Skills appear under the server's Codex home.
- User guide: [Trusted Mac-to-Linux Skill Migration](../server-migration.md).

### Task 4.6 - Mac-To-Linux Migration Acceptance

- Status: pending.
- Outcome: the first user completes the real Mac-to-Linux-server migration with
  documented transfer and conflict expectations.
- Automated verification: full Rust/frontend suites, deterministic large-bundle
  invocation counts, production frontend build, and package checks.
- Human verification: export real eligible Skills, transfer by an existing
  user-chosen method, ask server-side Codex to install them, and confirm no
  different existing Skill or credential was silently overwritten.

## Risks And Mitigations

- Archive attacks: strict entry classes, path normalization, streamed absolute
  limits, exact manifest matching, and failure cleanup.
- Secret leakage: deterministic filename/content checks before export, visible
  blocked evidence without secret bytes, and plain warning that v1 is not
  encrypted.
- Source races: preview revisions are advisory; export rechecks the exact
  directory immediately before reading and before commit.
- Target races: import classifications are advisory; Installation Confirmation
  performs fresh targeted ownership, conflict, and revision checks.
- Replacement interruption: personal replacement uses same-filesystem atomic
  exchange and preserves the old directory during boundary verification. A
  process crash in the narrow post-exchange window can leave a hidden recovery
  directory until cleanup; a durable startup recovery journal remains release
  hardening work and is not silently treated as a successful receipt.
- Format lock-in: keep v1 minimal and strict, record current alternatives, and
  add a format Adapter only when a maintained second format exists.
- Large-catalog latency: stream once, reuse verified snapshots, and update only
  affected catalog entries after Studio-owned mutations.

## Verification Matrix

| Area | Automated | Human |
| --- | --- | --- |
| Format and integrity | deterministic valid/adversarial fixtures | inspect manifest and receipt |
| Export eligibility | source, secret, race, no-overwrite tests | select and export disposable Skills |
| Import containment | traversal/link/bomb/cleanup tests | cancel review and inspect catalog |
| Comparison | complete revision and all-source conflict fixtures | compare identical and divergent Skill |
| Installation | stale/no-replace/managed/recovery tests | explicitly install selected revisions |
| Migration | deterministic export fixtures | real Mac-to-Linux workflow |

## Acceptance Record

- Phase 4 product outcome was accepted as part of the v0.1 brief on 2026-07-30.
- The owner initially accepted a portable CLI direction on 2026-08-04, then
  simplified the real requirement on 2026-08-06: a trusted Mac export is handed
  to server-side Codex for direct installation, without repeated semantic audit
  or a custom CLI. Exact duplicate skipping and active-personal export remain.
- Task 4.1 completed on 2026-08-05 with 18 deterministic format and adversarial
  fixture tests, a clean workspace Clippy run, a locked dependency check, the
  full project test/build command, and independent security and fixture-gap
  reviews. This task adds no desktop workflow for human clicking; GUI export
  begins in Task 4.2.
- Tasks 4.2 and 4.3 were implemented on 2026-08-05 with canonical export,
  deterministic secret blocking, source/destination race gates, verified
  contained staging, baseline review, file preview, and discard cleanup. Their
  final automated pass covers 89 desktop-core tests, 24 Bundle-core tests, 12
  frontend state/document tests, a production frontend build, workspace Clippy
  with warnings denied, and a locked workspace check. The owner accepted both
  desktop workflows on 2026-08-05 and authorized Task 4.4 implementation.
- Task 4.4 implementation reached automated acceptance on 2026-08-05 and is
  accepted by the owner after the desktop workflow pass on 2026-08-06. Import
  Classification preserves
  every same-name Catalog Match, Import Comparison reads only review-bound
  files, and Installation Confirmation consumes backend-issued offers in a
  separate action. Apply-time authorization uses one fail-closed cross-source
  scan, prepared-content revision checks, no-replace creation, and atomic
  directory exchange with boundary revision verification for personal
  replacement on macOS/Linux. Multi-Skill receipts retain each completed,
  skipped, or failed outcome. The final automated pass covers 105 desktop-core
  tests, 24 Bundle-core tests, 15 frontend tests, production frontend build,
  warnings-denied workspace Clippy, locked workspace check, and diff checks.
- Task 4.5's desktop handoff was implemented on 2026-08-06: a successful export
  now presents a copyable, file-name-only server instruction for trusted
  self-migration. Prompt tests verify that it skips repeated semantic audit,
  forbids script execution, skips identical content, and asks before replacing
  different server content. The owner accepted this export-receipt workflow on
  2026-08-06. Real Linux-server validation remains pending under Task 4.6.

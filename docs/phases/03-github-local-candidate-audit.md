# Phase 3: GitHub And Local Candidate Audit

## Context

Phase 2 now provides local Skill authoring, evidence, lifecycle guards, and
optional cloud review. Phase 3 extends the same evidence and staging model to
untrusted public candidates without reproducing a Skill distributor or a
security scanner.

## Scope

- Public GitHub Skill URL acquisition at a fixed commit.
- Local directory acquisition into contained temporary staging.
- Candidate file, hash, ownership, compatibility, and audit presentation.
- Optional cloud Deep Audit of explicitly confirmed staged text files.
- Optional adapter invocation for maintained external scanners.
- Separate explicit installation after review.

## Non-Goals

- Private GitHub credentials or GitHub Enterprise hosts.
- Generic Git protocol support, submodules, symlink installation, or arbitrary
  repository checkout.
- Executing candidate scripts, Git hooks, package managers, MCP servers, or
  Agent commands.
- Reimplementing `npx skills`, Cisco Skill Scanner, Snyk Agent Scan, or another
  maintained distribution/scanning product.

## Decisions And Alternatives

Task 3.1 research is recorded in
[github-acquisition-options.md](../research/github-acquisition-options.md).
The selected direction is a Rust GitHub snapshot adapter using commit/tree
metadata and fixed blob-SHA downloads through the GitHub API, pending the
owner's explicit acceptance.
The owner accepted this direction on 2026-08-01.

## Task Breakdown

### Task 3.1 - Validate GitHub Acquisition Approach

- Status: completed and accepted on 2026-08-01.
- Evidence: existing products and maintained Git implementations were compared;
  GitHub commit/tree/raw behavior was exercised against a public repository.

### Task 3.2 - Contained Candidate Staging

- Status: completed on 2026-08-01.
- Outcome: GitHub and local candidates become temporary, hash-verified staged
  manifests without touching managed Skill roots.
- Verification: traversal, symlink, submodule, duplicate, size, depth, race,
  cleanup, redirect, and fixed-SHA fixtures.

### Task 3.3 - Candidate Review Presentation

- Status: completed and accepted on 2026-08-04.
- Outcome: a non-programmer can see repository, requested ref, resolved commit,
  exact path, files, hashes, compatibility, skipped entries, and findings.

### Task 3.4 - Explicit Installation

- Status: completed and accepted on 2026-08-04.
- Outcome: installation is a separate confirmation with ownership,
  containment, destination conflict, and source compatibility checks.
- Verification: the apply path rechecks the staged file set and every file hash,
  refreshes cross-source conflicts immediately before commit, preserves
  executable modes, serializes Studio-owned mutations, and atomically renames
  without replacing the destination. Installation tests cover preview side
  effects, stale revisions, staged changes, late conflicts, blocking documents,
  repeated installation, byte preservation, and executable permissions.

### Task 3.5 - Candidate Cloud Deep Audit

- Status: completed and accepted on 2026-08-04.
- Outcome: a staged candidate can use the configured cloud provider through the
  same explicit file-list consent used by authoring. Consent binds the API mode,
  derived endpoint, model, session candidate hash, and selected file hashes;
  changing or discarding staging invalidates the preview. Audit remains
  side-effect free and never installs or executes candidate content.
- Verification: preview and run independently reverify the complete staging
  manifest. Consent binds the provider fingerprint, full staged revision,
  eligible upload-set hash, and each selected path plus SHA-256. Sensitive,
  binary, unsupported, oversized, excess-count, and excess-total files are
  excluded with visible reasons; root `SKILL.md` remains mandatory. Cloud
  findings are shown separately and remain advisory to the explicit install
  action.

### Task 3.6 - External Scanner Evidence Adapter

- Status: completed and accepted on 2026-08-04.
- Outcome: maintained scanners can contribute findings to the existing evidence
  model without becoming required dependencies or executing candidate content.
- Design: a private Rust manager creates a side-effect-free, revision-bound
  Scanner Plan before any external work. A Studio-owned Runtime is responsible
  for contained materialization, fixed dispatch, timeout, cancellation, output
  bounds, cleanup, and non-execution of candidate files. Maintained adapters can
  describe one fixed scanner configuration and parse its output, but cannot
  launch processes, contact networks, or expose arbitrary command settings.
- Evidence: the plan binds the Candidate Skill hash, exact file paths, sizes,
  SHA-256 values, executable modes, scanner and ruleset versions, execution and
  configuration hashes, a readable configuration summary, data destination,
  selected files, and timeout. Studio code validates paths, scanned-file
  identities, line ranges, evidence limits, finding namespaces, and derives the
  verdict from normalized findings rather than trusting a scanner verdict.
- Boundary: no real scanner or desktop control is registered in Task 3.6. MCP
  scanning is excluded from this interface and remains a separate future
  consented and sandboxed path. Baseline Audit, cloud Deep Audit, and
  Installation Confirmation remain independent.
- Verification: focused tests cover side-effect-free preview, stale candidates,
  scanner and configuration changes before execution, cancellation before
  execution, exact executable-mode identity, duplicate and ungrounded findings,
  invalid line ranges, evidence limits, external data destinations, one Runtime
  and parser call per accepted run, manager-owned raw-result hashing, and a real
  staged executable fixture that is never executed.

## Risks And Mitigations

- Moving refs: resolve and record the commit SHA before downloading.
- Archive/clone execution: use metadata plus fixed blob bytes; never invoke Git or a
  package manager during acquisition.
- Path confusion: canonicalize URL paths, validate tree entries, and enforce
  temporary-root containment on every write.
- API limits: minimize REST calls, bound concurrency, surface reset metadata,
  and never silently retry another host or ref.
- Resource exhaustion: enforce tree, file, depth, per-file, total-byte, and
  response limits before and during download.
- Scanner side effects: invoke only a local staged directory through an explicit
  adapter contract; MCP scanning remains a separate consented/sandboxed path.

## Verification Matrix

| Area | Automated | Human |
| --- | --- | --- |
| Fixed ref and SHA | commit/tree fixtures | inspect displayed commit |
| Path safety | traversal, duplicate, symlink, submodule fixtures | review skipped entries |
| Resource limits | oversized tree/file/total fixtures | clear limit message |
| Side effects | no managed-root writes, no process execution, cleanup | cancel and inspect catalog |
| Installation | separate conflict/ownership tests | approve a disposable Skill |
| Candidate Deep Audit | provider/candidate consent binding and stale-session fixtures | review exact outgoing files, cancel, then run |

## Acceptance Record

- Task 3.1 research completed on 2026-08-01. The implementation direction is
  accepted by the owner and Task 3.2 may proceed.
- Task 3.2 completed on 2026-08-01 with an app-owned temporary staging module,
  fixed-SHA GitHub transport, local file identity checks, bounded manifests,
  explicit discard commands, and focused Rust fixtures. Candidate staging does
  not write managed Skill roots or execute candidate content.
- Task 3.3 was implemented on 2026-08-02. The desktop flow stages a public
  GitHub URL or user-selected local folder, presents source/version/file/hash,
  Codex compatibility, baseline findings, and text previews from the staged
  manifest only, then discards the session when the review closes. The owner
  accepted the presentation on 2026-08-04.
- Task 3.4 was implemented on 2026-08-03. Installation now has its own preview,
  explicit confirmation, final full-manifest verification, fresh cross-source
  conflict check, no-replace atomic commit, and targeted catalog update. The
  owner accepted the disposable-candidate installation flow on 2026-08-04.
- Task 3.5 was implemented on 2026-08-04. The candidate review now reuses the
  configured provider and explicit outgoing-file consent, while binding the
  complete staged revision and selected file hashes before either model call.
  The owner accepted candidate acquisition, result display, Deep Audit, and
  install-after-review behavior on 2026-08-04.
- Task 3.6 was accepted on 2026-08-04. The private external-scanner seam binds
  exact candidate, scanner, configuration, execution, data-handling, and file
  identities; centralizes bounded execution behind a Studio Runtime; grounds
  normalized evidence against verified bytes; and excludes MCP scanning from
  the ordinary adapter path. No real scanner dependency or desktop control was
  added.

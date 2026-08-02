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
metadata and fixed-SHA raw downloads, pending the owner's explicit acceptance.
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

- Status: implemented on 2026-08-02; pending first-user acceptance.
- Outcome: a non-programmer can see repository, requested ref, resolved commit,
  exact path, files, hashes, compatibility, skipped entries, and findings.

### Task 3.4 - Explicit Installation

- Status: pending
- Outcome: installation is a separate confirmation with ownership,
  containment, destination conflict, and source compatibility checks.

### Task 3.5 - External Scanner Evidence Adapter

- Status: pending
- Outcome: maintained scanners can contribute findings to the existing evidence
  model without becoming required dependencies or executing candidate content.

## Risks And Mitigations

- Moving refs: resolve and record the commit SHA before downloading.
- Archive/clone execution: use metadata plus raw bytes; never invoke Git or a
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
  manifest only, then discards the session when the review closes. It awaits
  first-user visual acceptance before Task 3.4 starts.

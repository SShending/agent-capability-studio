# Agent Skill Studio

## Status

Accepted product brief. The product direction, v0.1 boundary, implementation
order, and recommended technical direction were approved on 2026-07-30.

## Goal

Give non-programmers a local desktop workspace where they can understand and
manage Agent capabilities without using a terminal or editing configuration
files by hand, beginning with safe authoring, auditing, comparison, and
migration of Agent Skills.

## Problem

Agent Skills are plain files with powerful instructions, scripts, permissions,
and trigger behavior. Existing tools help people discover, install, update, or
scan them, but a non-programmer still struggles to answer basic questions:

- What will this Skill do and when will it trigger?
- What changed between the installed version, a draft, and an imported copy?
- Which files, commands, network destinations, or credentials can it touch?
- Can it be moved to another machine or Agent without breaking?
- How can a new Skill be created safely without learning the file format?

The product should make those questions understandable while preserving exact
source, evidence, and user control.

## First User

The first user is the project owner, using Codex on macOS. They manage a growing
personal Skill collection, want to move Skills to a server, and prefer a polished
GUI over terminal commands. The interface must remain usable by people with no
programming background.

## Core Workflows

1. Browse personal, disabled, system, plugin-managed, and archived Skills with
   clear ownership and trigger-strategy labels.
2. Open a personal Skill in guided or source mode, edit a draft, review findings
   and exact differences, then explicitly save or discard it.
3. Create a Skill through a guided flow for purpose, trigger behavior, workflow,
   supporting files, and target Agent compatibility.
4. Inspect one or more Skill candidates discovered from a public GitHub
   repository, or one candidate from a local folder, before installation;
   optionally run a user-configured cloud semantic review after confirming the
   exact content and destination, while keeping audit and installation separate.
5. Export user-controlled Skills as a manifest-and-hash bundle. Desktop import
   verifies, stages, and compares a Bundle; for the owner's trusted Mac export,
   Codex on the Linux server may install it directly without repeating semantic
   audit.
6. Disable, re-enable, archive, restore, or permanently delete personal Skills
   through explicit, conflict-aware lifecycle actions.
7. Understand compatibility and conflicts without modifying system or
   plugin-managed Skills.

## Success Evidence

- A macOS user can install and open a signed desktop application without Node.js,
  `npm`, or terminal setup.
- A non-programmer can create or edit a valid Codex Skill and understand why it
  triggers.
- Every save or installation presents exact changes and relevant findings before
  mutation.
- Audit never installs, enables, deletes, or executes an untrusted Skill.
- Baseline Audit never uses the network. Deep Audit sends only explicitly
  confirmed content to the user's configured cloud model and identifies that
  provider in its result.
- Concurrent edits are detected before overwrite; path traversal and unsafe
  archive entries are rejected.
- Export moves eligible Skills from the local Mac to the owner's Linux server;
  Codex can install that trusted self-export while skipping existing identical
  content and asking before replacing a different same-name Skill.
- System and plugin-managed Skills remain read-only and excluded from export.
- Personal Skill lifecycle actions never overwrite another directory; permanent
  deletion is available only from archive after an explicit destructive
  confirmation.
- The normal workflow creates no persistent HTML reports or cleanup burden.

## Constraints

- Product: keep v0.1 focused on Skill authoring, evidence, comparison, and
  migration. Add another Agent capability type only after the Skill workflow is
  stable, current alternatives are re-evaluated, and a shared Studio model is
  demonstrated; do not become an undifferentiated configuration manager.
- User experience: local GUI first, plain language, familiar macOS interaction,
  advanced source and evidence available one level deeper.
- Safety: distinguish evidence from guarantees; never label a Skill "safe" or
  "secure". Show finding, severity, confidence, and exact evidence.
- Agency: treat explicit-name and contextual intent triggers as valid strategies.
  Never batch-rewrite trigger policy. Require confirmation for overwrites,
  conflicts, installation, and findings needing manual review.
- Filesystem: validate and contain every read, extraction, move, and write. Use
  hashes for optimistic concurrency and atomic replacement for saved files.
- Privacy: operate locally by default. Cloud Deep Audit is opt-in, user
  configured, and confirmed per scan with the destination and files shown. Do
  not upload credentials, unrelated files, prior results, or usage data.
- Reports: keep audit results ephemeral unless the user explicitly exports one.
- Compatibility: implement Codex first behind an Agent compatibility seam; add
  other Agents only after their contracts are tested.
- License: release the project under MIT.

## Non-Goals For The Initial Product

- Rebuilding CC Switch installation, update, synchronization, shared-store, or
  broad cross-Agent distribution features.
- Building another general Skill/MCP security scanner.
- Reimplementing MCP Inspector protocol testing and debugging.
- Managing MCP, rules, plugins, hooks, automations, or runtime tool-call traces in
  v0.1.
- Supporting Claude Code, OpenClaw, Hermes, Windows, and Linux before the Codex
  desktop workflow is stable.
- Executing untrusted Skill scripts as part of an audit.
- A hosted Skill marketplace, rankings, or cloud account system.

## Existing Solutions

Research was checked against current public project documentation during July
2026. Recheck material claims before major scope expansion.

- **CC Switch**: mature Tauri desktop management across Codex, Claude Code,
  OpenCode, OpenClaw, Hermes, and others. It covers discovery, GitHub/ZIP install,
  search, filtering, update detection, backup/restore, shared storage, and
  distribution. Use or integrate it for those jobs rather than reproducing them.
- **Vercel Skills CLI / skills.sh**: covers discovery, registry workflows,
  installation, updates, and audit aggregation. It is not the target authoring
  and explanation experience.
- **MCPJam**: includes Skill discovery, upload, frontmatter validation, and
  playground injection. Current evidence does not establish the same guided
  non-programmer authoring, revision comparison, and migration workflow.
- **Cisco AI Defense Skill Scanner** and **Snyk Agent Scan**: cover mature static
  and Agent supply-chain scanning concerns. Integrate their evidence where useful;
  do not recreate their engines.
- **Official MCP Inspector**: owns interactive MCP server testing and debugging.
  A future MCP assistant should launch or explain it, not clone it.

## Defensible Differentiation

Build a Skill Studio, not another package manager or scanner: guided authoring,
plain-language evidence, exact revision comparison, explicit mutation gates, and
portable migration for people who do not work directly with files. Existing
tools can remain discovery, distribution, scanning, or debugging backends.

## Long-Term Product Horizon

- Treat Skills as the first Agent capability type, not the permanent limit of
  the product.
- Candidate capability types include Agent instruction/rule files, MCP
  configuration and explanation, plugins, hooks, commands, declared tools and
  permissions, and privacy-controlled runtime tool-call evidence.
- Keep specialized jobs with maintained products: use MCP Inspector for protocol
  testing, CC Switch for broad distribution and synchronization, and established
  scanners for security-engine evidence.
- Admit a capability type only when it can reuse the Studio's proven interaction
  model: discover ownership, explain behavior, edit or author, show evidence and
  exact differences, and guard every mutation.
- Reconsider the `Agent Skill Studio` name only after a second capability type is
  implemented and validated with the first user.

## Current Starting Point

- A Tauri 2 macOS application discovers real Codex Skill locations through a
  Rust core and ships without a Node HTTP runtime.
- Personal Skills support source and heading-aware guided editing, deterministic
  baseline audit, exact diff display, stale-hash conflict detection, and atomic
  save. System and plugin-managed Skills remain read-only.
- Optional Deep Audit supports a user-configured OpenAI-compatible provider,
  explicit Chat Completions or Responses mode, a passwordless app-private local
  credential file with an explicitly documented same-user threat boundary,
  per-run file consent, and two-pass grounded semantic review without tool access.
- The unsigned local `.app` and Phase 1 workflow were accepted by the first
  user. Guided creation, candidate audit, bundle migration, signing, and public
  release remain in later phases.
- The audit is intentionally a small baseline and must not be presented as a
  replacement for a maintained security scanner.

## Recommended Technical Direction

- Package the product as a Tauri 2 desktop application, macOS first.
- Reuse the current interaction model and visual language, but replace the Node
  HTTP filesystem layer with typed Tauri commands; do not ship a hidden Node
  server or require Node.js on the user's machine.
- Keep filesystem discovery, containment, hashing, atomic writes, bundle parsing,
  and adapter-specific placement in the Rust desktop core.
- Keep the Skill Bundle format and verification core free of Tauri and WebView
  dependencies so desktop export/import remains testable and portable. Do not
  build a separate headless Bundle CLI unless a real migration proves that
  Codex-assisted installation and existing transfer tools are insufficient.
- Keep presentation, guided authoring, evidence explanation, and comparison in a
  web frontend hosted by the Tauri WebView.
- Preserve the audit interface so deterministic built-in checks and optional
  external scanner adapters return the same evidence model.
- Keep cloud semantic analysis behind a provider adapter. Treat Skill content as
  untrusted data, disable tool execution, require structured evidence grounded
  in submitted files, and run an independent false-positive review before
  aggregation.
- Prove the architecture with one end-to-end vertical slice before porting every
  existing management action.

## Proposed v0.1 Boundary

1. Signed macOS Tauri application and local Codex discovery.
2. Browse and inspect Skills with ownership and trigger-strategy labels.
3. Guided/source editing, baseline audit, diff, guarded save, and discard.
4. Guided creation of a new Codex Skill.
5. Skill Bundle export, import verification/staging, conflict comparison, and
   explicit installation.
6. Optional launch or import of evidence from maintained scanners; no custom
   full security engine.

## Confirmed Decisions

- Use Tauri 2 with macOS-first delivery; cross-platform support follows a stable
  Codex desktop workflow.
- Implement in this order: desktop vertical slice, guided creation, GitHub/local
  candidate audit, then Skill Bundle import/export.
- Keep GitHub and local candidate audit in v0.1.
- Define an external-scanner adapter seam in v0.1 without requiring a scanner
  integration or recreating a scanning engine.
- Let the user configure the cloud model used for optional Deep Audit; never
  reuse Codex login credentials or silently send Skill content.
- Use an OpenAI-compatible API Base URL, explicit Chat Completions or Responses
  mode, and model name for the first cloud profile. Store its API key separately
  in the app configuration directory with strict current-user-only permissions;
  never expose it to the frontend, logs, projects, Skills, or Bundles.
- Allow unsigned local development builds; sign and notarize the public v0.1
  release.
- Exclude credentials from Skill Bundles and do not add bundle encryption in
  v0.1.
- Treat repeated Bundle Import and installation as idempotent: an exact Skill
  revision already present in any managed source is shown and skipped rather
  than copied or overwritten.
- Treat a Bundle exported by the owner from this Mac as trusted self-migration
  input on the owner's Linux server. Do not require a second semantic audit;
  have Codex install it without silently replacing different existing content.
- Use `Agent Skill Studio` as the product name during the Skill-first product
  stage. The public repository is `agent-capability-studio`, leaving room for
  later validated capability modules without prematurely renaming the product.
- Treat Agent Skills as the first capability module; validate one second
  capability type after v0.1 before expanding or renaming the product.
- Target macOS 13 or later provisionally; verify the final minimum against Tauri,
  WebKit, signing, and tested feature requirements before release.
- Use Rust 1.85 or later for development so the portable core can retain current
  maintained dependencies without forcing unrelated Tauri dependency downgrades.

## Open Decisions

- Phase 3 must select a maintained, contained GitHub acquisition approach
  through Task 3.1 before candidate staging implementation begins.

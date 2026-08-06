# Skill Bundle Format Options

Research date: 2026-08-04.

## Decision

Use a strict, versioned ZIP container that keeps the normal Agent Skills
directory layout and adds only the missing transport manifest. Do not reuse a
CC Switch synchronization artifact, Vercel Skills lock file, or generic source
archive as the Studio's migration contract.

The defensible difference remains narrow: the Studio exports selected
eligible user-controlled Skills with exact file evidence, excludes likely credentials,
verifies and stages an imported bundle, compares it with every managed Codex
source, and requires a separate Installation Confirmation. It does not provide
registry discovery, updates, cloud synchronization, or remote transfer.

For the first user's Mac-to-Linux-server workflow, do not add a separate
headless Bundle CLI. The Bundle is a trusted self-export, existing tools can
transfer it, and server-side Codex can install the contained Skill directories
without repeating semantic audit. It must ask before replacing different
same-name server content. Reconsider a custom CLI only after a real migration
shows that this simpler path is insufficient.

## Current Evidence

### Agent Skills

The open Agent Skills specification defines a portable Skill directory rooted
at `SKILL.md`, with optional scripts, references, assets, and Agent metadata. It
does not currently define a multi-Skill archive manifest, per-file transport
hashes, import staging, or target conflict semantics.

Reuse: preserve the Skill directories byte-for-byte inside the bundle and keep
Agent-specific compatibility outside the container format.

Evidence inspected at `agentskills/agentskills` main commit
`27a9f0c075e876ad632fc2e88b8866c5dc8ca15c`.

### Vercel Skills CLI

The CLI installs from repositories, local directories, individual Skill files,
and generic ZIP/TAR downloads. Its archive reader has valuable defensive
behavior for traversal, duplicate entries, links, encryption, compression,
central-directory consistency, checksums, and resource limits. Its v3 lock file
records installation provenance and a Skill folder hash.

Neither the documented CLI workflow nor the inspected archive and lock-file
implementation defines a portable multi-Skill export format with a manifest of
every file, staged import, and Studio conflict classifications.

Reuse: follow its bounded archive-validation lessons. Do not copy its Node
implementation or treat its installation lock file as portable Skill content.

Evidence inspected at `vercel-labs/skills` main commit
`1164afa5f0e21ebd01e6fc11249759353f494ad1`, especially `src/archive.ts`,
`src/skill-lock.ts`, and the README.

### CC Switch

CC Switch documents unified Skill management, GitHub/ZIP installation,
shared storage, automatic Skill backups, and cloud synchronization. Its primary
ownership model includes a SQLite database and app-specific synchronization
state. The inspected public README and repository tree do not document a stable
standalone Skill archive contract with per-file SHA-256 identities and the
Studio's verify-stage-compare-confirm workflow.

Reuse or integrate CC Switch for broad cross-Agent distribution and
synchronization. Reusing an internal backup or sync artifact would couple the
Studio to CC Switch state and duplicate the part of the product explicitly kept
out of scope.

Evidence inspected at `farion1231/cc-switch` main commit
`40b6376b2adfefef90b34df61006416c2ee5c030`.

## Alternatives

### Generic ZIP Without A Manifest

Rejected. It is convenient transport, but cannot establish the expected file
set, executable modes, complete Skill revisions, or whether an extra file was
injected.

### Content-Addressed Object Archive

Rejected for v1. Storing files by hash would deduplicate repeated assets and
prepare for signatures, but it makes manual inspection and implementation more
complex before either capability is required. The manifest-and-directory
layout already provides exact integrity and a clean future migration path.

### TAR Or TAR.GZ

Rejected for v1. TAR brings link, device, ownership, PAX, and sparse-file
semantics that the Studio must reject, while offering a less familiar macOS
artifact for the first user. ZIP is not inherently safe, but a maintained Rust
codec plus strict entry and extraction checks gives the narrower surface.

## Uncertainty

This is a point-in-time inspection of the named public repositories and their
documented interfaces, not proof that no third-party bundle convention exists.
Recheck the Agent Skills specification and maintained distribution tools before
publishing a stable format beyond v0.1. If a compatible standard emerges,
prefer a format adapter or upstream contribution over parallel evolution.

## Sources

- https://github.com/agentskills/agentskills
- https://agentskills.io/specification
- https://github.com/vercel-labs/skills
- https://github.com/farion1231/cc-switch

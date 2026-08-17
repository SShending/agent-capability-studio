# Agent Skill Studio

Agent Skill Studio is a macOS desktop workspace for people who want to
understand, edit, audit, compare, and migrate Codex Skills without managing
Skill files by hand.

[简体中文说明](README.zh-CN.md)

## What it does

- Finds personal, disabled, archived, system, and plugin-managed Codex Skills.
- Creates and edits a complete Skill package, including `SKILL.md`,
  `references/`, `scripts/`, `assets/`, and other package files.
- Shows plain-language evidence for triggers, destructive commands, network
  access, sensitive-data signals, execution, persistence, dependencies, and
  encoded content before a mutation.
- Organizes Skills with Collections, provenance, package validation, and
  GitHub comparison.
- Imports a local Skill or a public GitHub repository, lists conventional Skills
  in a multi-Skill repository, and lets the user choose candidates separately.
- Exports selected user-controlled Skills as a verified Bundle for a trusted
  Mac-to-Linux migration. Identical Skills are skipped; different same-name
  Skills require confirmation before replacement.
- Disables, restores, archives, and permanently deletes personal Skills only
  through explicit lifecycle confirmations. System and plugin-managed Skills
  remain read-only.

## Audit and privacy

Baseline Audit is local and offline. It is a bounded evidence check, not a
security certificate or a guarantee that a Skill is harmless.

Deep Audit is optional. Before it runs, the app shows the selected API mode,
derived endpoint, model, and exact files that will leave the Mac. It supports
OpenAI-compatible Chat Completions and Responses modes and never silently
falls back to another protocol. The confirmed files are sent in two requests:
a threat review and an independent false-positive review. The provider receives
no tools and the app accepts only evidence that can be checked against the
submitted files.

Provider, endpoint, and model preferences are stored separately from the API
key. The API key is stored in an app-private local file with restrictive
permissions (`0700` directory and `0600` files on Unix systems), atomic writes,
and symlink/permission checks. Opening Settings checks only whether a valid key
exists; it does not read the key or require a password. This is not equivalent
to Keychain protection against malicious software already running as the same
macOS account. Keys are never written to Skills, Bundles, project files, logs,
or audit evidence.

## Install and run

When a signed release is published, download its DMG from GitHub Releases,
open it, and drag **Agent Skill Studio** to Applications. The public release
must pass Developer ID signing, notarization, stapling, and clean-machine
installation checks before it is published.

For development or an unsigned local build:

```bash
npm ci
npm run desktop:dev
npm run desktop:build
```

The shipped app does not start a Node HTTP server and does not require Node.js
or Rust on the user's machine. Node.js and Rust are development dependencies;
the repository pins Node `22.23.1` and Rust `1.88.0` for reproducible builds.

## Trusted migration

For a Skill Bundle exported by the owner from this Mac:

1. Export the selected Skills in the Studio.
2. Transfer the `.skillbundle` with a trusted method such as SFTP or the
   server provider's file-transfer UI.
3. Open Codex in the directory containing the Bundle and paste the installation
   instruction from the export receipt.
4. Confirm only different same-name Skills that should replace the server copy.

This self-migration does not repeat semantic audit, but it still verifies the
Bundle, prevents traversal, avoids duplicate identical Skills, and never
silently overwrites different content. See
[`docs/server-migration.md`](docs/server-migration.md).

## Product boundary

The Studio is Codex-first and Skill-first. It does not replace CC Switch's
cross-Agent distribution and synchronization, MCP Inspector's protocol
debugging, or maintained Skill/MCP security scanners. Additional Agent
adapters and capability types require a separate compatibility contract and
validation before they are added.

## Contributing and security

Read [`AGENTS.md`](AGENTS.md), [`INIT.md`](INIT.md), [`CONTEXT.md`](CONTEXT.md),
and [`PLAN.md`](PLAN.md) for project constraints and roadmap. Please report
security issues privately according to [`SECURITY.md`](SECURITY.md), rather
than opening a public issue with exploit details.

## License

MIT. See [`LICENSE`](LICENSE).

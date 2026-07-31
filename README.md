# Agent Skill Studio

A macOS desktop workspace for understanding, editing, auditing, comparing, and
migrating Agent Skills. It is designed for people who do not want to edit Skill
files by hand.

## Run

```bash
npm install
npm run desktop:dev
```

For a local unsigned application build:

```bash
npm run desktop:build
```

The built app is at `src-tauri/target/release/bundle/macos/Agent Skill Studio.app`.
The shipped application does not start a Node HTTP server or require Node.js.

## Current scope

- Discover personal, disabled, system, plugin-managed, and archived Codex Skills.
- Edit personal Skills through a guided form or the `SKILL.md` source.
- Inspect draft structure, trigger scope, high-impact commands, network access,
  sensitive-data signals, command execution, persistence, dependency installs,
  encoded payloads, and exact changes before saving.
- Require explicit confirmation before saving findings that need manual review.
- Optionally run a two-pass semantic Deep Audit through a user-configured
  OpenAI-compatible provider using either Chat Completions or Responses after
  confirming the exact endpoint and files that will leave the machine; store
  its API key in macOS Keychain.
- Open global Settings from the toolbar or with `Command+,` to configure the
  Deep Audit provider and protected credential.
- Prevent stale drafts from overwriting a Skill changed elsewhere.
- Disable, re-enable, archive, and restore personal Skills without overwriting
  destination directories.
- Permanently delete only archived Skills after typing the exact Skill name.
- Keep system and plugin-managed Skills read-only.

Audit results are evidence-based findings, not a security certificate or an
absolute safety guarantee. Baseline Audit stays offline. Deep Audit sends the
confirmed files to the configured provider twice: threat review and independent
false-positive review.

## Product boundary

Agent Skill Studio does not replace CC Switch, MCP Inspector, or dedicated
security scanners. Its focus is guided Skill authoring, understandable evidence,
version comparison, and migration. Codex is the first Agent adapter; additional
Agent adapters are planned after the core workflow is stable.

## License

MIT

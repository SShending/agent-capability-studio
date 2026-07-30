# Agent Skill Studio

A Codex-first visual workspace for understanding, editing, auditing, comparing,
and migrating Agent Skills. It is designed for people who do not want to edit
Skill files by hand.

## Run

```bash
npm install
npm start
```

Open `http://127.0.0.1:4177`.

The server binds only to `127.0.0.1`.

## Current scope

- Discover personal, disabled, system, plugin-managed, and archived Codex Skills.
- Edit personal Skills through a guided form or the `SKILL.md` source.
- Inspect draft structure, trigger scope, high-impact commands, network access,
  sensitive-data signals, and exact changes before saving.
- Require explicit confirmation before saving findings that need manual review.
- Prevent stale drafts from overwriting a Skill changed elsewhere.
- Enable, disable, archive, restore, validate, and install Skills from GitHub.

Audit results are evidence-based findings, not a security certificate or an
absolute safety guarantee. System and plugin-managed Skills remain read-only.

## Product boundary

Agent Skill Studio does not replace CC Switch, MCP Inspector, or dedicated
security scanners. Its focus is guided Skill authoring, understandable evidence,
version comparison, and migration. Codex is the first Agent adapter; additional
Agent adapters are planned after the core workflow is stable.

## License

MIT

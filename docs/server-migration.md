# Trusted Mac-To-Linux Skill Migration

This workflow is for a Bundle exported by the owner from their own Mac. It is
not the workflow for a Bundle downloaded from an unknown source.

## Workflow

1. In Agent Skill Studio, export the enabled personal Skills that should move to
   the server.
2. Transfer the resulting `.skillbundle` file with an existing trusted method,
   such as `scp`, SFTP, or the server provider's file transfer UI.
3. In the export receipt, click **Copy server installation instruction**.
4. Open Codex in the directory containing the transferred Bundle and paste the
   instruction.
5. Confirm only same-name Skills whose server content is different. Identical
   content is skipped.

Codex may extract the Bundle into a temporary directory and copy its `skills/`
directories into the current Codex home. It does not need to repeat semantic
audit because this is the owner's trusted self-export. It must not execute
bundled scripts or silently replace different server content.

## Expected Result

- New Skills appear in the server's personal Codex Skill directory.
- Identical Skills are not duplicated.
- Different same-name Skills remain unchanged until the owner confirms which
  version to keep.
- Codex reports installed, skipped, and unresolved items.

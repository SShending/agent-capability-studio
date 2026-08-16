# ADR 0003: Repository Intake URL Scheme

- Status: superseded before implementation
- Date: 2026-08-16

The Studio needs a no-terminal entry point for a repository author to offer a
repository containing many Skills. We will register the stable custom macOS
URL scheme `agent-capability-studio://` for Repository Intake. The protocol
name intentionally remains stable if the desktop product later changes from
Agent Skill Studio to Agent Capability Studio; choosing it does not authorize
that product rename today.

Links carry only an untrusted public GitHub source and optional requested ref.
Opening a link launches the installed app but does not make a network request,
run an Audit, or install anything. After explicit user confirmation, the app
resolves the requested ref or default branch to an immutable commit SHA, applies the Repository Intake
discovery rules, and lets the user choose which Candidate Skills to stage.
The resulting listing and queue remain bound to that SHA even if the remote ref
moves; seeing a newer commit requires an explicit refresh or new intake.
Unsupported schemes, non-public or credential-bearing sources, arbitrary
commands, provider settings, and automatic-mutation flags are rejected.

## Superseded

The existing **Import Skill** workflow already accepts an ordinary GitHub URL
and routes the result into Candidate Review. Phase 7 therefore extends that
workflow to enumerate multiple Skills in one repository instead of adding a
second entry point. The custom scheme is not part of the accepted Phase 7
scope. It may be reconsidered only when a real external producer, such as a
maintained browser integration or repository-hosted control, has been validated.

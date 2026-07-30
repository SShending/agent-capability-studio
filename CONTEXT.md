# Agent Skill Studio

This context describes the language for a Codex-first workspace where people
can understand, create, edit, audit, compare, and migrate Agent Skills without
confusing inspection, transport, and installation.

## Language

### Skill authoring

**Skill Draft**:
A user-owned, editable Skill that may not yet be installed or ready for use.
_Avoid_: Template, candidate

**Skill Authoring**:
The act of creating or substantially reshaping a Skill Draft around a user's
goal, trigger conditions, workflow, and supporting files.
_Avoid_: Skill generation, prompt writing

**Skill Revision**:
A saved version of a Skill Draft or installed Skill, identified by its content
and its relationship to an earlier version.
_Avoid_: Edit session, update

**Installed Skill**:
A Skill that has been copied or linked into an Agent's recognized Skill scope
and is available for that Agent to use.
_Avoid_: Enabled Skill (unless describing runtime activation)

### Inspection and installation

**Candidate Skill**:
A Skill submitted for pre-installation audit from a public GitHub source or a
local directory.
_Avoid_: Download, package

**Audit Result**:
The evidence, findings, confidence, and verdict produced for one exact Candidate
Skill version.
_Avoid_: Security certificate, safety guarantee

**Verdict**:
One of three evidence-based conclusions: no blocking findings, manual review
required, or blocking recommended.
_Avoid_: Safe, secure

**Audit**:
An evidence-gathering inspection of a specific Skill revision. An Audit does
not install, enable, modify, or remove the Skill.
_Avoid_: Approval, certification

**Installation Confirmation**:
The user's explicit decision to place an audited Skill revision into a target
Agent scope. An Audit Result or a Bundle Import never implies Installation
Confirmation.
_Avoid_: Accept, deploy

**Exportable Skill**:
A user-controlled Skill eligible for migration. Codex-managed system Skills and
plugin-managed Skills are not Exportable Skills.
_Avoid_: Every installed Skill

### Personal Skill lifecycle

**Lifecycle Action**:
An explicit user decision that moves a user-controlled Skill between personal,
disabled, and archive states without changing its contents.
_Avoid_: Audit, installation, automatic cleanup

**Disabled Skill**:
A user-controlled Skill retained locally outside the active personal scope, so
new Agent tasks do not load it.
_Avoid_: Deleted Skill, archived Skill

**Archived Skill**:
A user-controlled Skill removed from active and disabled scopes but retained for
restoration or permanent deletion.
_Avoid_: Disabled Skill, backup

**Restore**:
A Lifecycle Action that returns an Archived Skill to the active personal scope.
_Avoid_: Bundle Import, installation

**Permanent Deletion**:
The irreversible removal of an Archived Skill after an exact destructive
confirmation. It is never an Audit outcome or an automatic action.
_Avoid_: Archive, disable, cleanup


**Bundle Import**:
Verification and staging of a Skill Bundle on a target environment. Bundle
Import does not by itself mean the Skills have been installed.
_Avoid_: Restore, install

### Portability

**Agent Adapter**:
A compatibility boundary that describes how this product discovers, validates,
previews, and places Skills for one Agent. Codex is the first Agent Adapter;
other Agents are separate adapters.
_Avoid_: Agent mode, integration

**Target Scope**:
The specific user, project, or system location in an Agent where an Installed
Skill can be placed and used.
_Avoid_: Environment, destination folder

**Skill Compatibility**:
The evidence that a Skill revision's format, trigger semantics, and referenced
files can be understood by a Target Scope through an Agent Adapter.
_Avoid_: Universal support, portable by default

**Skill Bundle**:
A portable collection of Exportable Skills plus a manifest that identifies and
hashes every included Skill and file.
_Avoid_: Backup, installer

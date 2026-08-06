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

**Candidate Source**:
The public GitHub location or user-selected local directory from which one
Candidate Skill revision is obtained.
_Avoid_: Install source, managed Skill root

**Staged Candidate**:
A temporary, contained snapshot of one Candidate Skill prepared for inspection
but not installed into any Agent scope.
_Avoid_: Installed Skill, checkout, cache

**Candidate Manifest**:
The source revision, paths, sizes, modes, and hashes that identify one exact
Staged Candidate.
_Avoid_: Audit Result, installation receipt

**Audit Result**:
The evidence, findings, confidence, and verdict produced for one exact Skill
revision, including the named analyzers and cloud provider when applicable.
_Avoid_: Security certificate, safety guarantee

**Verdict**:
One of three evidence-based conclusions: no blocking findings, manual review
required, or blocking recommended.
_Avoid_: Safe, secure

**Destructive Data Intent**:
An instruction directing an Agent to erase a broad set of user, project, or
system data, whether or not it contains a recognizable shell command.
_Avoid_: Routine cleanup, dangerous-command substring

**Audit**:
An evidence-gathering inspection of a specific Skill revision. An Audit does
not install, enable, modify, or remove the Skill.
_Avoid_: Approval, certification

**Baseline Audit**:
A deterministic, local, offline Audit that provides immediate structural and
high-signal evidence but does not claim broad semantic coverage.
_Avoid_: Deep Audit, complete scan

**Deep Audit**:
An explicit Audit that combines maintained scanner evidence with a
user-configured cloud semantic review of confirmed Skill files.
_Avoid_: Automatic upload, Baseline Audit

**External Scanner Adapter**:
A Studio-owned mapping from one maintained scanner and supported result format
into the shared evidence model. It is not a general command or plugin system.
_Avoid_: Scanner plugin SDK, arbitrary command adapter

**Scanner Plan**:
A side-effect-free, revision-bound description of the scanner identity,
configuration, data handling, and exact Candidate Skill files to be inspected.
_Avoid_: Scan result, scanner configuration

**Scanner Audit Contribution**:
Grounded findings from one Scanner Plan for one exact Candidate Skill revision,
kept separate from Installation Confirmation and the complete Audit Result.
_Avoid_: Scanner approval, security certificate

**Cloud Model Profile**:
The user-owned provider destination, model selection, and protected credential
used only for confirmed Deep Audits.
_Avoid_: Codex login, application account

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

**Bundle Manifest**:
The versioned list of Skill directories and exact file identities carried by a
Skill Bundle.
_Avoid_: Archive index, Agent configuration

**Import Classification**:
The primary relationship of one imported Skill revision to the target catalog:
new, identical, user conflict, managed conflict, or incompatible.
_Avoid_: Installation decision, Audit verdict

**Catalog Match**:
One installed, disabled, archived, system-managed, or plugin-managed Skill whose
canonical name matches an imported Skill, retained as comparison evidence.
_Avoid_: Primary conflict, overwrite target

**Import Comparison**:
Revision and file-level evidence showing how one imported Skill differs from its
Catalog Matches before an Installation Confirmation.
_Avoid_: Diff approval, automatic merge

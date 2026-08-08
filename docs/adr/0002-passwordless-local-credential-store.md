# ADR 0002: Passwordless Local Deep Audit Credential Store

- Status: accepted
- Date: 2026-08-08

## Context

The initial Deep Audit implementation offered session-only memory or macOS
Keychain storage. Session storage required the owner to enter the API key after
every restart. Reading Keychain state while opening Settings could trigger a
macOS password prompt, even though opening Settings should not require access to
the credential itself. The owner rejected both behaviors and explicitly chose a
passwordless local alternative.

Without Keychain, a user-supplied master password, or another external hardware
root of trust, the application cannot protect a persistent secret from malicious
software already running as the same macOS account. Encrypting the credential
with a key stored beside it would not change that boundary and would imply
protection the product does not provide.

## Decision

- Persist API mode, endpoint, and model in the application configuration
  directory, separate from the API key.
- Persist the API key in a dedicated application-private file. On Unix systems,
  enforce `0700` on its parent directory and `0600` on the credential and
  preference files.
- Use atomic replacement for both files. Reject a credential path that is a
  symbolic link, is not a regular file, exceeds the fixed size limit, or is
  readable by group or other users.
- Opening Settings checks only whether a valid credential file exists. It does
  not read or return the credential. Only connection testing and a confirmed
  Deep Audit read it.
- Never return, display, log, export, or place the API key in a project, Skill,
  Bundle, audit finding, or frontend state.
- Do not automatically read or migrate the beta Keychain entry. That could
  recreate the authorization prompt this decision removes.
- Remove the storage-mode choice from the interface. Present one accurate local
  storage explanation rather than asking non-programmers to select an
  implementation mechanism.

## Consequences

The provider profile and API key survive application restarts without a password
prompt. Other local accounts cannot read the credential through normal file
permissions, and common accidental exposure paths remain excluded.

This is weaker than Keychain against malware or another process running as the
same account. The interface and public documentation must state that limitation
and must not call the store encrypted, secure, or equivalent to Keychain. A
future cross-platform release needs an operating-system-specific permission
review before reusing this adapter on Windows or Linux.

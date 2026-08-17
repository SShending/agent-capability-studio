# Release Engineering

This document is for the release owner. It describes how Agent Skill Studio
produces a macOS release candidate without exposing Apple credentials to pull
requests or publishing a GitHub Release automatically.

## Release contract

The v0.1 release configuration is intentionally narrow:

- application version `0.1.0`;
- exact Node.js `22.23.1` and Rust `1.88.0` build toolchains;
- Rust MSRV `1.88`, matching the minimum required by the locked dependency set;
- universal `arm64` and `x86_64` macOS application;
- `.app` and `.dmg` bundle targets;
- an explicit macOS application and DMG volume icon;
- macOS 13.0 packaging floor;
- hardened runtime, Developer ID signing, Apple notarization, and stapling for
  the public candidate.

The 13.0 floor is enforced in repository configuration and artifact metadata.
It remains the provisional public minimum until Task 5.4 validates the exact
candidate on a clean supported Mac.

Run the repository contract check after changing a version or release setting:

```sh
npm run release:check
```

For a tagged candidate, the tag must be `v` followed by the package version and
must point to the checked-out commit:

```sh
npm run release:check -- --release-tag v0.1.0
```

## Trust boundary

`.github/workflows/ci.yml` runs on pushes and pull requests without repository
or environment secrets. It installs locked dependencies, runs frontend and Rust
checks, builds a universal application and DMG with signing explicitly disabled,
and verifies their version, minimum macOS value, and architectures. Its output
is test evidence, not a distributable public release.

`.github/workflows/release-candidate.yml` is a separate manual workflow. Its job
uses the protected GitHub environment named `release`, checks out an exact
existing version tag, signs and notarizes through Tauri, verifies the produced
artifacts, and uploads only the DMG and its SHA-256 file as a temporary workflow
artifact. It does not create, edit, or publish a GitHub Release.

## GitHub environment and secrets

Create an environment named `release` in the repository settings before running
the candidate workflow. Configure a required reviewer and restrict deployment
to the intended release tags or protected branch. Environment protection is the
human authorization boundary for credential access.

Store these values as environment secrets, not repository files:

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded Developer ID Application `.p12` certificate and private key |
| `APPLE_CERTIFICATE_PASSWORD` | Password used when exporting the `.p12` |
| `APPLE_API_KEY` | App Store Connect API key ID |
| `APPLE_API_ISSUER` | App Store Connect issuer ID |
| `APPLE_API_KEY_P8` | Complete contents of the matching private `.p8` file |

Tauri imports the certificate from `APPLE_CERTIFICATE` and infers its Developer
ID Application identity. The workflow writes the `.p8` value only to the GitHub
runner's temporary directory with mode `0600`, passes its path through
`APPLE_API_KEY_PATH`, and relies on runner cleanup after the job. No release
step prints these values.

Do not put certificates, API keys, passwords, `.env` files, exported bundles,
or generated installers in Git. The repository ignore rules are only a backstop;
inspect staged changes before every release-related commit.

## Build and verification

For local or ordinary CI packaging, install both Rust targets and explicitly use
the unsigned command:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm ci
npm run release:check
npm run release:build:unsigned
npm run release:verify:unsigned
```

The signed command is reserved for the protected candidate workflow because it
requires the Apple secrets above:

```sh
npm run release:build:signed
npm run release:verify:signed
```

Signed verification fails unless all of the following are true:

- there is exactly one application and one DMG;
- the application version and macOS floor match repository metadata;
- the application contains `Contents/Resources/icon.icns`;
- the executable contains both required architectures;
- the application and DMG have Developer ID Application signatures and a Team
  Identifier;
- Apple stapling validation and Gatekeeper assessment accept both artifacts.

Only after those checks pass does the verifier write the DMG SHA-256 file.
Rerunning verification replaces that generated checksum with the value computed
from the currently verified DMG.

## Producing a candidate

1. Finish the intended release commit and run the full local checks.
2. Create and push the exact version tag, such as `v0.1.0`.
3. In GitHub Actions, run **Release candidate** and enter that tag.
4. Approve the protected `release` environment deployment.
5. Download the temporary workflow artifact and retain its DMG and checksum
   together for Task 5.4 acceptance.

The owner must still complete Task 4.6 migration evidence and Task 5.4
clean-machine installation, launch, core-workflow, upgrade, and removal checks.
Publication remains a separate explicit action after those gates pass.

## Current local limitation

Repository checks and unsigned universal packaging do not require Apple
credentials. A real Developer ID signature, notarization submission, staple,
and Gatekeeper acceptance can only be validated when the protected environment
has the owner's Apple certificate and App Store Connect API key. Do not record
those release checks as passed until the signed candidate workflow supplies the
artifact evidence.

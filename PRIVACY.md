# Privacy Notes

Agent Skill Studio is designed to keep ordinary Skill management on the Mac.
It has no account system, telemetry, hosted sync service, or background upload.

## What stays local

Discovery, editing, package validation, Collections, provenance records,
Baseline Audit, lifecycle operations, Bundle export, and local comparison stay
on the Mac. The app does not execute Skill scripts as part of these workflows.

Provider preferences are stored in the app configuration directory. The API key
is stored separately in an app-private credential file with restrictive Unix
permissions and atomic replacement. Keys are not included in Bundles or logs.

## What can leave the Mac

Only an explicitly confirmed Deep Audit sends data to the endpoint and model
shown in the consent screen. The consent screen lists the exact selected files.
The provider receives those files in two requests for threat review and
independent false-positive review. Connection tests send only a fixed synthetic
prompt and never read Skill files.

GitHub import and update checks contact the public GitHub endpoints needed to
resolve the requested repository and download the selected candidate. They do
not send local Skill content to GitHub. Trusted Bundle migration uses a transfer
method chosen by the user, such as SFTP; the Studio does not operate a hosted
transfer service.

## User choices and limits

Deep Audit is optional and local editing does not depend on it. Cancellation
before consent performs no Deep Audit network request. The app does not claim
that an empty audit result means a Skill is safe or secure. Users should review
the exact evidence, source, revision, and provider before making a decision.

## Retention

The Studio does not retain cloud responses in a hosted service. Local audit
state and provider configuration are kept only as needed by the application and
can be cleared from Settings or by removing the app's local data. Provider-side
retention is controlled by the configured provider; consult that provider's
privacy policy before enabling Deep Audit.

import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const ciWorkflow = new URL("../.github/workflows/ci.yml", import.meta.url);
const candidateWorkflow = new URL(
  "../.github/workflows/release-candidate.yml",
  import.meta.url
);

test("pull-request CI builds and verifies universal artifacts without credentials", async () => {
  const workflow = await readFile(ciWorkflow, "utf8");

  assert.match(workflow, /pull_request:/);
  assert.match(workflow, /npm run release:build:unsigned/);
  assert.match(workflow, /npm run release:verify:unsigned/);
  assert.doesNotMatch(workflow, /secrets\./);
  assert.doesNotMatch(workflow, /APPLE_(?:CERTIFICATE|API_)/);
});

test("release candidates require the protected release environment and exact tag", async () => {
  const workflow = await readFile(candidateWorkflow, "utf8");

  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /environment:\s*release/);
  assert.match(workflow, /npm run release:check -- --release-tag/);
  assert.match(workflow, /npm run release:build:signed/);
  assert.match(workflow, /npm run release:verify:signed/);
  assert.match(workflow, /APPLE_CERTIFICATE:\s*\$\{\{ secrets\.APPLE_CERTIFICATE \}\}/);
  assert.match(workflow, /APPLE_API_KEY_PATH=/);
  assert.match(workflow, /actions\/upload-artifact@/);
  assert.match(workflow, /actions\/checkout@[0-9a-f]{40}/);
  assert.match(workflow, /actions\/setup-node@[0-9a-f]{40}/);
  assert.match(workflow, /actions\/upload-artifact@[0-9a-f]{40}/);
  assert.ok(
    workflow.indexOf("Verify tagged release configuration")
      < workflow.indexOf("Prepare App Store Connect API key")
  );
  assert.doesNotMatch(workflow, /(?:gh release|tauri-action|softprops\/action-gh-release)/i);
});

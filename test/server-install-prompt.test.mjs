import test from "node:test";
import assert from "node:assert/strict";
import { bundleFileName, serverInstallPrompt } from "../src/server-install-prompt.js";

test("uses only the transferred Bundle file name in the server prompt", () => {
  assert.equal(
    bundleFileName("/Users/example/Desktop/my-skills.skillbundle"),
    "my-skills.skillbundle"
  );
  assert.equal(
    bundleFileName("C:\\Users\\example\\my-skills.skillbundle"),
    "my-skills.skillbundle"
  );
});

test("trusted migration prompt skips repeated audit and prevents silent overwrite", () => {
  const prompt = serverInstallPrompt("/tmp/codex-skills.skillbundle");

  assert.match(prompt, /可信技能迁移包/);
  assert.doesNotMatch(
    prompt.replaceAll("Agent Skill Studio", ""),
    /\b(?:Skills?|Bundles?)\b/
  );
  assert.match(prompt, /不需要重新做语义安全审查/);
  assert.match(prompt, /不要执行其中的脚本/);
  assert.match(prompt, /内容相同.*跳过/);
  assert.match(prompt, /内容不同.*询问我.*不要覆盖/);
});

test("trusted migration prompt follows the selected interface language", () => {
  const prompt = serverInstallPrompt("/tmp/codex-skills.skillbundle", "en");

  assert.match(prompt, /trusted Bundle/);
  assert.match(prompt, /Do not repeat the semantic security audit/);
  assert.match(prompt, /do not run its scripts/);
  assert.match(prompt, /identical content/);
  assert.match(prompt, /ask me before replacing/);
});

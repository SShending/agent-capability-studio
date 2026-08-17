import test from "node:test";
import assert from "node:assert/strict";
import { chmod, mkdtemp, mkdir, utimes, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { directoryRevision } from "../scripts/release-artifact-model.mjs";

async function applicationFixture(root, script = "binary") {
  const application = join(root, "Agent Skill Studio.app");
  await mkdir(join(application, "Contents", "MacOS"), { recursive: true });
  await mkdir(join(application, "Contents", "Resources"), { recursive: true });
  await writeFile(join(application, "Contents", "Info.plist"), "plist");
  await writeFile(join(application, "Contents", "MacOS", "agent-skill-studio"), script);
  await chmod(join(application, "Contents", "MacOS", "agent-skill-studio"), 0o755);
  await writeFile(join(application, "Contents", "Resources", "icon.icns"), "icon");
  return application;
}

test("application revisions bind complete contents and executable modes but ignore mtimes", async () => {
  const root = await mkdtemp(join(tmpdir(), "agent-skill-studio-app-revision-"));
  const first = await applicationFixture(join(root, "first"));
  const second = await applicationFixture(join(root, "second"));

  await utimes(join(second, "Contents", "Info.plist"), new Date(1), new Date(1));
  assert.equal(await directoryRevision(first), await directoryRevision(second));

  await writeFile(join(second, "Contents", "Resources", "extra.txt"), "extra");
  assert.notEqual(await directoryRevision(first), await directoryRevision(second));

  const third = await applicationFixture(join(root, "third"));
  await chmod(join(third, "Contents", "MacOS", "agent-skill-studio"), 0o644);
  assert.notEqual(await directoryRevision(first), await directoryRevision(third));

  const fourth = await applicationFixture(join(root, "fourth"), "different binary");
  assert.notEqual(await directoryRevision(first), await directoryRevision(fourth));
});

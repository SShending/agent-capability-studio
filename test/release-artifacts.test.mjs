import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { access, copyFile, mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const releaseVerifier = new URL("../scripts/verify-release-artifacts.mjs", import.meta.url);

async function emptyBundle() {
  const root = await mkdtemp(join(tmpdir(), "agent-skill-studio-artifacts-"));
  const bundle = join(root, "bundle");
  await mkdir(join(bundle, "macos", "Agent Skill Studio.app"), { recursive: true });
  await mkdir(join(bundle, "dmg"), { recursive: true });
  return bundle;
}

async function unsignedBundle() {
  const root = await mkdtemp(join(tmpdir(), "agent-skill-studio-artifacts-"));
  const bundle = join(root, "bundle");
  const contents = join(bundle, "macos", "Agent Skill Studio.app", "Contents");
  const executableDirectory = join(contents, "MacOS");
  const resourcesDirectory = join(contents, "Resources");
  await mkdir(executableDirectory, { recursive: true });
  await mkdir(resourcesDirectory, { recursive: true });
  await mkdir(join(bundle, "dmg"), { recursive: true });
  await copyFile("/usr/bin/true", join(executableDirectory, "agent-skill-studio"));
  await writeFile(join(resourcesDirectory, "icon.icns"), "fixture icon");
  await writeFile(join(contents, "Info.plist"), `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>agent-skill-studio</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
</dict></plist>
`);
  await writeFile(join(bundle, "dmg", "Agent Skill Studio_0.1.0_universal.dmg"), "fixture");
  await mkdir(join(root, "src-tauri"), { recursive: true });
  await writeFile(join(root, "package.json"), JSON.stringify({ version: "0.1.0" }));
  await writeFile(join(root, "src-tauri", "tauri.conf.json"), JSON.stringify({
    version: "0.1.0",
    bundle: { macOS: { minimumSystemVersion: "13.0" } }
  }));
  return { bundle, root };
}

function verify(bundle, mode = "unsigned", projectRoot = null) {
  const args = [
    releaseVerifier.pathname,
    "--bundle-dir",
    bundle,
    "--mode",
    mode
  ];
  if (projectRoot) args.push("--project-root", projectRoot);
  return spawnSync(process.execPath, [
    ...args
  ], { encoding: "utf8" });
}

test("release verification rejects a bundle without exactly one DMG", async () => {
  const bundle = await emptyBundle();

  const result = verify(bundle);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /expected exactly one DMG artifact, found 0/);
});

test("unsigned release verification rejects an invalid DMG after checking app metadata", async () => {
  const fixture = await unsignedBundle();

  const result = verify(fixture.bundle, "unsigned", fixture.root);

  assert.notEqual(result.status, 0);
  assert.match(result.stdout, /version 0\.1\.0, macOS 13\.0, universal x86_64\+arm64/);
  assert.match(result.stderr, /mounting DMG failed/);
});

test("signed release verification rejects unsigned artifacts before writing a checksum", async () => {
  const fixture = await unsignedBundle();
  const checksum = join(
    fixture.bundle,
    "dmg",
    "Agent Skill Studio_0.1.0_universal.dmg.sha256"
  );

  const result = verify(fixture.bundle, "signed", fixture.root);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /application Developer ID signature verification failed/);
  await assert.rejects(access(checksum), { code: "ENOENT" });
});

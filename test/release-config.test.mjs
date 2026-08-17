import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const releaseCheck = new URL("../scripts/check-release-config.mjs", import.meta.url);
const projectRoot = fileURLToPath(new URL("..", import.meta.url));

async function writeReleaseFixture({
  packageVersion = "0.1.0",
  tauriVersion = packageVersion,
  cargoVersion = packageVersion,
  nodeVersion = "22.23.1",
  rustVersion = "1.88.0",
  license = "MIT",
  bundleTargets = ["app", "dmg"],
  bundleIcons = ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"],
  minimumSystemVersion = "13.0",
  hardenedRuntime = true,
  unsignedBuild = "tauri build --ci --no-sign --target universal-apple-darwin --bundles app,dmg",
  signedBuild = "tauri build --ci --target universal-apple-darwin --bundles app,dmg"
} = {}) {
  const root = await mkdtemp(join(tmpdir(), "agent-skill-studio-release-"));
  await mkdir(join(root, "src-tauri", "src"), { recursive: true });
  await writeFile(join(root, ".nvmrc"), `${nodeVersion}\n`);
  await writeFile(join(root, "rust-toolchain.toml"), [
    "[toolchain]",
    `channel = "${rustVersion}"`,
    'profile = "minimal"',
    'components = ["clippy", "rustfmt"]',
    ""
  ].join("\n"));
  await writeFile(join(root, "package.json"), JSON.stringify({
    name: "agent-skill-studio",
    version: packageVersion,
    license,
    repository: "https://github.com/example/agent-skill-studio",
    engines: { node: nodeVersion },
    scripts: {
      "release:build:unsigned": unsignedBuild,
      "release:build:signed": signedBuild
    }
  }));
  await writeFile(join(root, "src-tauri", "tauri.conf.json"), JSON.stringify({
    productName: "Agent Skill Studio",
    version: tauriVersion,
    identifier: "com.tahanan.agent-skill-studio",
    bundle: {
      active: true,
      targets: bundleTargets,
      icon: bundleIcons,
      category: "DeveloperTool",
      shortDescription: "A visual workspace for Agent Skills.",
      longDescription: "Understand and manage Agent Skills.",
      macOS: { minimumSystemVersion, hardenedRuntime }
    }
  }));
  await writeFile(join(root, "src-tauri", "Cargo.toml"), [
    "[package]",
    'name = "agent-skill-studio"',
    `version = "${cargoVersion}"`,
    'edition = "2021"',
    'rust-version = "1.88"',
    `license = "${license}"`,
    'repository = "https://github.com/example/agent-skill-studio"',
    ""
  ].join("\n"));
  await writeFile(join(root, "src-tauri", "src", "main.rs"), "fn main() {}\n");
  return root;
}

function runReleaseCheck(root, releaseTag = null) {
  const args = [releaseCheck.pathname, "--root", root];
  if (releaseTag) args.push("--release-tag", releaseTag);
  return spawnSync(process.execPath, args, {
    encoding: "utf8"
  });
}

function git(root, args) {
  const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
}

test("release configuration rejects mismatched application versions", async () => {
  const root = await writeReleaseFixture({ tauriVersion: "0.1.1" });

  const result = runReleaseCheck(root);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /package, Tauri, and Cargo versions must match/);
});

test("repository release configuration declares the public macOS contract", () => {
  const result = runReleaseCheck(projectRoot);

  assert.equal(result.status, 0, result.stderr);
  assert.match(
    result.stdout,
    /version 0\.1\.0, macOS 13\.0, bundles app\+dmg, Node 22\.23\.1, Rust 1\.88\.0/
  );
});

test("release configuration reports incomplete public distribution metadata", async () => {
  const root = await writeReleaseFixture({
    nodeVersion: "22",
    rustVersion: "stable",
    license: "UNLICENSED",
    bundleTargets: ["app"],
    minimumSystemVersion: "12.0",
    hardenedRuntime: false
  });

  const result = runReleaseCheck(root);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /license metadata must be MIT/);
  assert.match(result.stderr, /bundle targets must be exactly app and dmg/);
  assert.match(result.stderr, /minimum macOS version must be 13\.0/);
  assert.match(result.stderr, /macOS hardened runtime must be enabled/);
  assert.match(result.stderr, /Node version must be an exact release/);
  assert.match(result.stderr, /rust-toolchain\.toml must pin Rust 1\.88\.0/);
});

test("release configuration keeps signed and unsigned build commands distinct", async () => {
  const root = await writeReleaseFixture({
    unsignedBuild: "tauri build --ci --target universal-apple-darwin --bundles app,dmg",
    signedBuild: "tauri build --ci --no-sign --target universal-apple-darwin --bundles app,dmg"
  });

  const result = runReleaseCheck(root);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /unsigned release build must explicitly disable signing/);
  assert.match(result.stderr, /signed release build must not disable signing/);
});

test("release configuration declares the macOS application icon", async () => {
  const root = await writeReleaseFixture({ bundleIcons: ["icons/icon.ico"] });

  const result = runReleaseCheck(root);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /bundle icon configuration must include icons\/icon\.icns/);
});

test("release tag verification binds the declared version to the checked-out commit", async () => {
  const root = await writeReleaseFixture();
  git(root, ["init", "--quiet"]);
  git(root, ["config", "user.name", "Release Test"]);
  git(root, ["config", "user.email", "release@example.invalid"]);
  git(root, ["add", "."]);
  git(root, ["commit", "--quiet", "-m", "release fixture"]);
  git(root, ["tag", "v0.1.0"]);

  const accepted = runReleaseCheck(root, "v0.1.0");
  const rejected = runReleaseCheck(root, "v0.1.1");

  assert.equal(accepted.status, 0, accepted.stderr);
  assert.match(accepted.stdout, /release tag v0\.1\.0 points to the checked-out commit/i);
  assert.notEqual(rejected.status, 0);
  assert.match(rejected.stderr, /release tag must equal v0\.1\.0/);
});

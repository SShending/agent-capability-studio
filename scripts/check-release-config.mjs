import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

function argumentValue(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? null : process.argv[index + 1] ?? null;
}

function repositoryUrl(repository) {
  return typeof repository === "string" ? repository : repository?.url;
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

const root = resolve(argumentValue("--root") ?? ".");
const releaseTag = argumentValue("--release-tag");
const packageJson = await readJson(resolve(root, "package.json"));
const tauriConfig = await readJson(resolve(root, "src-tauri", "tauri.conf.json"));
const nodeVersion = (await readFile(resolve(root, ".nvmrc"), "utf8")).trim();
const rustToolchain = await readFile(resolve(root, "rust-toolchain.toml"), "utf8");
const rustVersion = rustToolchain.match(/^channel\s*=\s*"([^"]+)"/m)?.[1];
const cargoResult = spawnSync("cargo", [
  "metadata",
  "--format-version",
  "1",
  "--no-deps",
  "--manifest-path",
  resolve(root, "src-tauri", "Cargo.toml")
], { encoding: "utf8" });

if (cargoResult.status !== 0) {
  console.error("Release configuration invalid:\n- Cargo metadata could not be read");
  process.exit(1);
}

const cargoMetadata = JSON.parse(cargoResult.stdout);
const cargoPackage = cargoMetadata.packages.find(({ name }) => name === packageJson.name);
const versions = [packageJson.version, tauriConfig.version, cargoPackage?.version];
const errors = [];

if (!cargoPackage || new Set(versions).size !== 1) {
  errors.push("package, Tauri, and Cargo versions must match");
}

const bundleTargets = Array.isArray(tauriConfig.bundle?.targets)
  ? [...tauriConfig.bundle.targets].sort()
  : [];
const bundleIcons = Array.isArray(tauriConfig.bundle?.icon)
  ? tauriConfig.bundle.icon
  : [];
const unsignedBuild = packageJson.scripts?.["release:build:unsigned"] ?? "";
const signedBuild = packageJson.scripts?.["release:build:signed"] ?? "";
const universalBundlePattern = /--target\s+universal-apple-darwin.*--bundles\s+app,dmg/;
if (bundleTargets.join(",") !== "app,dmg") {
  errors.push("Tauri bundle targets must be exactly app and dmg");
}
if (tauriConfig.bundle?.macOS?.minimumSystemVersion !== "13.0") {
  errors.push("the minimum macOS version must be 13.0");
}
if (tauriConfig.bundle?.macOS?.hardenedRuntime !== true) {
  errors.push("the macOS hardened runtime must be enabled");
}
if (!bundleIcons.includes("icons/icon.icns")) {
  errors.push("bundle icon configuration must include icons/icon.icns");
}
if (!/(?:^|\s)--no-sign(?:\s|$)/.test(unsignedBuild)) {
  errors.push("the unsigned release build must explicitly disable signing");
}
if (/(?:^|\s)--no-sign(?:\s|$)/.test(signedBuild)) {
  errors.push("the signed release build must not disable signing");
}
if (
  !universalBundlePattern.test(unsignedBuild)
  || !universalBundlePattern.test(signedBuild)
) {
  errors.push("release builds must target universal app and dmg bundles");
}
if (packageJson.engines?.node !== nodeVersion) {
  errors.push("package engines.node must match .nvmrc");
}
if (rustVersion !== "1.88.0") {
  errors.push("rust-toolchain.toml must pin Rust 1.88.0");
}
if (!/^\d+\.\d+\.\d+$/.test(nodeVersion)) {
  errors.push("Node version must be an exact release");
}
if (packageJson.license !== "MIT" || cargoPackage?.license !== "MIT") {
  errors.push("package and Cargo license metadata must be MIT");
}
const packageRepository = repositoryUrl(packageJson.repository)?.replace(/\.git$/, "");
if (
  !packageRepository?.startsWith("https://github.com/")
  || packageRepository !== cargoPackage?.repository?.replace(/\.git$/, "")
) {
  errors.push("package and Cargo repository metadata must match an HTTPS GitHub URL");
}
if (cargoPackage?.rust_version !== "1.88") {
  errors.push("Cargo rust-version must match the locked dependency floor of 1.88");
}
if (
  tauriConfig.productName !== "Agent Skill Studio"
  || tauriConfig.identifier !== "com.tahanan.agent-skill-studio"
  || tauriConfig.bundle?.active !== true
  || tauriConfig.bundle?.category !== "DeveloperTool"
  || !tauriConfig.bundle?.shortDescription
  || !tauriConfig.bundle?.longDescription
) {
  errors.push("Tauri public product metadata is incomplete");
}
if (releaseTag) {
  const expectedTag = `v${packageJson.version}`;
  if (releaseTag !== expectedTag) {
    errors.push(`release tag must equal ${expectedTag}`);
  } else {
    const head = spawnSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" });
    const tagCommit = spawnSync(
      "git",
      ["rev-parse", "--verify", `refs/tags/${releaseTag}^{commit}`],
      { cwd: root, encoding: "utf8" }
    );
    if (
      head.status !== 0
      || tagCommit.status !== 0
      || head.stdout.trim() !== tagCommit.stdout.trim()
    ) {
      errors.push(`release tag ${releaseTag} must point to the checked-out commit`);
    }
  }
}

if (errors.length > 0) {
  console.error(`Release configuration invalid:\n${errors.map((error) => `- ${error}`).join("\n")}`);
  process.exit(1);
}

console.log(
  `Release configuration OK: version ${packageJson.version}, macOS 13.0, `
    + `bundles app+dmg, Node ${nodeVersion}, Rust ${rustVersion}`
);
if (releaseTag) {
  console.log(`Release tag ${releaseTag} points to the checked-out commit`);
}

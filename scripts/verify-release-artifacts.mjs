import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { createReadStream } from "node:fs";
import { access, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { directoryRevision } from "./release-artifact-model.mjs";

function argumentValue(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? null : process.argv[index + 1] ?? null;
}

async function matchingEntries(directory, predicate) {
  try {
    return (await readdir(directory, { withFileTypes: true }))
      .filter(predicate)
      .map(({ name }) => resolve(directory, name));
  } catch {
    return [];
  }
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

function run(command, args, label) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${label} failed: ${(result.stderr || result.stdout).trim()}`);
  }
  return result.stdout.trim();
}

function plistValue(plist, key) {
  return run(
    "/usr/bin/plutil",
    ["-extract", key, "raw", "-o", "-", plist],
    `reading ${key} from Info.plist`
  );
}

function commandResult(command, args) {
  return spawnSync(command, args, { encoding: "utf8" });
}

function commandFailure(result) {
  const detail = (result.stderr || result.stdout).trim();
  return detail ? `: ${detail}` : "";
}

function verifyDeveloperIdSignature(path, label) {
  const verification = commandResult(
    "/usr/bin/codesign",
    ["--verify", "--deep", "--strict", "--verbose=2", path]
  );
  if (verification.status !== 0) {
    throw new Error(
      `${label} Developer ID signature verification failed${commandFailure(verification)}`
    );
  }

  const details = commandResult("/usr/bin/codesign", ["--display", "--verbose=4", path]);
  const signatureDetails = `${details.stdout}\n${details.stderr}`;
  if (
    details.status !== 0
    || !/^Authority=Developer ID Application:/m.test(signatureDetails)
    || !/^TeamIdentifier=\S+/m.test(signatureDetails)
  ) {
    throw new Error(`${label} is not signed by a Developer ID Application identity`);
  }
}

function verifyStaple(path, label) {
  const result = commandResult("/usr/bin/xcrun", ["stapler", "validate", path]);
  if (result.status !== 0) {
    throw new Error(`${label} notarization staple validation failed${commandFailure(result)}`);
  }
}

function verifyGatekeeper(path, label, type) {
  const args = type === "execute"
    ? ["--assess", "--type", "execute", "--verbose=4", path]
    : [
        "--assess",
        "--type",
        "open",
        "--context",
        "context:primary-signature",
        "--verbose=4",
        path
      ];
  const result = commandResult("/usr/sbin/spctl", args);
  if (result.status !== 0) {
    throw new Error(`${label} Gatekeeper assessment failed${commandFailure(result)}`);
  }
}

async function sha256(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

async function verifyDiskImageApplication(diskImage, application) {
  const mountDirectory = await mkdtemp(join(tmpdir(), "agent-skill-studio-dmg-"));
  let attached = false;
  let failure = null;
  let revision = null;

  try {
    run(
      "/usr/bin/hdiutil",
      ["attach", "-nobrowse", "-readonly", "-mountpoint", mountDirectory, diskImage],
      "mounting DMG"
    );
    attached = true;
    const mountedApplications = await matchingEntries(
      mountDirectory,
      (entry) => entry.isDirectory() && entry.name.endsWith(".app")
    );
    if (mountedApplications.length !== 1) {
      throw new Error(
        `expected exactly one application inside DMG, found ${mountedApplications.length}`
      );
    }

    const [applicationRevision, mountedRevision] = await Promise.all([
      directoryRevision(application),
      directoryRevision(mountedApplications[0])
    ]);
    if (mountedRevision !== applicationRevision) {
      throw new Error("application inside DMG does not match the verified application bundle");
    }
    revision = applicationRevision;
  } catch (error) {
    failure = error;
  }

  if (attached) {
    const detach = commandResult("/usr/bin/hdiutil", ["detach", mountDirectory]);
    if (detach.status !== 0 && !failure) {
      failure = new Error(`unmounting DMG failed${commandFailure(detach)}`);
    }
  }
  await rm(mountDirectory, { recursive: true, force: true });

  if (failure) throw failure;
  return revision;
}

const bundleDirectory = resolve(argumentValue("--bundle-dir") ?? "");
const projectRoot = resolve(argumentValue("--project-root") ?? ".");
const mode = argumentValue("--mode") ?? "signed";
if (!bundleDirectory || !["unsigned", "signed"].includes(mode)) {
  console.error("Usage: verify-release-artifacts --bundle-dir <path> --mode <unsigned|signed>");
  process.exit(1);
}

const applications = await matchingEntries(
  resolve(bundleDirectory, "macos"),
  (entry) => entry.isDirectory() && entry.name.endsWith(".app")
);
const diskImages = await matchingEntries(
  resolve(bundleDirectory, "dmg"),
  (entry) => entry.isFile() && entry.name.endsWith(".dmg")
);
const errors = [];

if (applications.length !== 1) {
  errors.push(`expected exactly one application artifact, found ${applications.length}`);
}
if (diskImages.length !== 1) {
  errors.push(`expected exactly one DMG artifact, found ${diskImages.length}`);
}

if (errors.length > 0) {
  console.error(`Release artifact verification failed:\n${errors.map((error) => `- ${error}`).join("\n")}`);
  process.exit(1);
}

const packageJson = await readJson(resolve(projectRoot, "package.json"));
const tauriConfig = await readJson(resolve(projectRoot, "src-tauri", "tauri.conf.json"));
const application = applications[0];
const plist = resolve(application, "Contents", "Info.plist");
const expectedVersion = packageJson.version;
const expectedMinimumMacOS = tauriConfig.bundle?.macOS?.minimumSystemVersion;
const artifactVersion = plistValue(plist, "CFBundleShortVersionString");
const artifactMinimumMacOS = plistValue(plist, "LSMinimumSystemVersion");
const executableName = plistValue(plist, "CFBundleExecutable");
const executable = resolve(application, "Contents", "MacOS", executableName);
const applicationIcon = resolve(application, "Contents", "Resources", "icon.icns");
const architectures = run("/usr/bin/lipo", ["-archs", executable], "reading application architectures")
  .split(/\s+/);

if (tauriConfig.version !== expectedVersion || artifactVersion !== expectedVersion) {
  errors.push("application artifact version does not match repository metadata");
}
if (artifactMinimumMacOS !== expectedMinimumMacOS) {
  errors.push("application minimum macOS version does not match Tauri configuration");
}
if (!architectures.includes("x86_64") || !architectures.some((arch) => /^arm64(?:e)?$/.test(arch))) {
  errors.push("application must contain x86_64 and arm64 architectures");
}
try {
  await access(applicationIcon);
} catch {
  errors.push("application bundle is missing Resources/icon.icns");
}

if (errors.length > 0) {
  console.error(`Release artifact verification failed:\n${errors.map((error) => `- ${error}`).join("\n")}`);
  process.exit(1);
}

console.log(
  `Release artifact metadata OK (${mode}): version ${artifactVersion}, macOS ${artifactMinimumMacOS}, `
    + "universal x86_64+arm64"
);

const diskImage = diskImages[0];
if (mode === "signed") {
  try {
    verifyDeveloperIdSignature(application, "application");
    verifyDeveloperIdSignature(diskImage, "DMG");
    verifyStaple(application, "application");
    verifyStaple(diskImage, "DMG");
    verifyGatekeeper(application, "application", "execute");
    verifyGatekeeper(diskImage, "DMG", "open");
    const applicationRevision = await verifyDiskImageApplication(diskImage, application);
    const checksum = await sha256(diskImage);
    const checksumPath = `${diskImage}.sha256`;
    await writeFile(checksumPath, `${checksum}  ${basename(diskImage)}\n`);
    console.log(
      `Signed release verification OK; DMG application ${applicationRevision}, `
        + `checksum written to ${checksumPath}`
    );
  } catch (error) {
    console.error(`Release artifact verification failed:\n- ${error.message}`);
    process.exit(1);
  }
} else {
  try {
    const applicationRevision = await verifyDiskImageApplication(diskImage, application);
    console.log(`DMG application matches verified bundle: ${applicationRevision}`);
  } catch (error) {
    console.error(`Release artifact verification failed:\n- ${error.message}`);
    process.exit(1);
  }
}

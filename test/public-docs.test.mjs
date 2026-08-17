import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const publicDocs = ["README.md", "README.zh-CN.md", "SECURITY.md", "PRIVACY.md", "LICENSE"];

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

test("public documentation files exist and local Markdown links resolve", () => {
  for (const file of publicDocs) assert.ok(fs.existsSync(path.join(root, file)), file);

  const markdownFiles = publicDocs.filter((file) => file.endsWith(".md"));
  for (const file of markdownFiles) {
    const source = read(file);
    for (const [, target] of source.matchAll(/\[[^\]]+\]\(([^)]+)\)/g)) {
      if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith("#")) continue;
      const cleanTarget = target.split("#", 1)[0];
      assert.ok(fs.existsSync(path.resolve(root, path.dirname(file), cleanTarget)), `${file}: ${target}`);
    }
  }
});

test("public documentation does not contain local paths or credential material", () => {
  const source = publicDocs.map(read).join("\n");
  assert.doesNotMatch(source, /\/(?:Users|private|var\/folders)\//);
  assert.doesNotMatch(source, /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/);
  assert.doesNotMatch(source, /(?:sk|rk)-[A-Za-z0-9_-]{20,}/);
  assert.doesNotMatch(source, /(?:APPLE_API_KEY_P8|APPLE_CERTIFICATE_PASSWORD)\s*[:=]\s*[^`\n]+/);
});

test("public metadata consistently declares the MIT package and current version", () => {
  const packageJson = JSON.parse(read("package.json"));
  const cargo = read("src-tauri/Cargo.toml");
  const tauri = JSON.parse(read("src-tauri/tauri.conf.json"));
  assert.equal(packageJson.license, "MIT");
  assert.match(cargo, /license\s*=\s*"MIT"/);
  assert.equal(packageJson.version, tauri.version);
  assert.equal(packageJson.version, "0.1.0");
});

test("README describes the passwordless local credential store accurately", () => {
  const readme = read("README.md");
  assert.match(readme, /app-private local file/);
  assert.match(readme, /not equivalent\s+to Keychain/);
  assert.doesNotMatch(readme, /store its API key in macOS Keychain/i);
});

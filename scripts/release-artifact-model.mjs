import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, readlink, readdir } from "node:fs/promises";
import { join } from "node:path";

function record(hash, ...parts) {
  for (const part of parts) hash.update(String(part)).update("\0");
}

export async function directoryRevision(root) {
  const hash = createHash("sha256");

  async function visit(path, relativePath) {
    const metadata = await lstat(path);
    const normalizedPath = relativePath || ".";

    if (metadata.isDirectory()) {
      record(hash, "directory", normalizedPath, metadata.mode & 0o777);
      const entries = (await readdir(path)).sort();
      for (const entry of entries) {
        const childPath = join(path, entry);
        const childRelativePath = relativePath ? `${relativePath}/${entry}` : entry;
        await visit(childPath, childRelativePath);
      }
      return;
    }

    if (metadata.isFile()) {
      record(hash, "file", normalizedPath, metadata.mode & 0o777, metadata.size);
      for await (const chunk of createReadStream(path)) hash.update(chunk);
      return;
    }

    if (metadata.isSymbolicLink()) {
      record(hash, "symlink", normalizedPath, await readlink(path));
      return;
    }

    throw new Error(`unsupported application entry type: ${normalizedPath}`);
  }

  await visit(root, "");
  return hash.digest("hex");
}

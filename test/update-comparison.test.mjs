import assert from "node:assert/strict";
import test from "node:test";
import { buildUpdateComparison } from "../src/update-comparison.js";

const file = (path, sha256, executable = false) => ({
  path,
  sha256,
  executable,
  size: 1
});

test("classifies added removed modified and unchanged update files", () => {
  const result = buildUpdateComparison(
    [
      file("SKILL.md", "local"),
      file("removed.txt", "removed"),
      file("same.txt", "same")
    ],
    [
      file("SKILL.md", "remote"),
      file("added.txt", "added"),
      file("same.txt", "same")
    ]
  );
  assert.deepEqual(
    result.map(({ path, status }) => [path, status]),
    [
      ["added.txt", "added"],
      ["removed.txt", "removed"],
      ["same.txt", "unchanged"],
      ["SKILL.md", "modified"]
    ]
  );
});

test("treats executable mode changes as modified", () => {
  const [result] = buildUpdateComparison(
    [file("scripts/run.sh", "same", false)],
    [file("scripts/run.sh", "same", true)]
  );
  assert.equal(result.status, "modified");
  assert.equal(result.modeChanged, true);
});

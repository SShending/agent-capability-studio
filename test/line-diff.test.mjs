import assert from "node:assert/strict";
import test from "node:test";
import { createLineDiff } from "../src/line-diff.js";

test("creates Git-style added removed and context rows with line numbers", () => {
  const diff = createLineDiff("one\ntwo\nthree\n", "one\nchanged\nthree\n");
  assert.equal(diff.truncated, false);
  assert.deepEqual(
    diff.rows.map(({ kind, oldLine, newLine, text }) => [kind, oldLine, newLine, text]),
    [
      ["context", 1, 1, "one"],
      ["remove", 2, null, "two"],
      ["add", null, 2, "changed"],
      ["context", 3, 3, "three"]
    ]
  );
});

test("collapses distant unchanged lines and bounds large comparisons", () => {
  const before = Array.from({ length: 20 }, (_, index) => `line ${index}`).join("\n");
  const after = before.replace("line 10", "changed");
  const diff = createLineDiff(before, after);
  assert.ok(diff.rows.some((row) => row.kind === "skip"));
  const large = Array.from({ length: 801 }, (_, index) => `${index}`).join("\n");
  assert.equal(createLineDiff(large, large).truncated, true);
});

const MAX_LINES = 800;
const CONTEXT_LINES = 3;

export function createLineDiff(before = "", after = "") {
  const beforeLines = splitLines(before);
  const afterLines = splitLines(after);
  if (beforeLines.length > MAX_LINES || afterLines.length > MAX_LINES) {
    return { truncated: true, rows: [] };
  }
  const width = afterLines.length + 1;
  const table = new Uint32Array((beforeLines.length + 1) * width);
  for (let left = beforeLines.length - 1; left >= 0; left -= 1) {
    for (let right = afterLines.length - 1; right >= 0; right -= 1) {
      const index = left * width + right;
      table[index] = beforeLines[left] === afterLines[right]
        ? table[(left + 1) * width + right + 1] + 1
        : Math.max(table[(left + 1) * width + right], table[left * width + right + 1]);
    }
  }
  const operations = [];
  let left = 0;
  let right = 0;
  let oldLine = 1;
  let newLine = 1;
  while (left < beforeLines.length || right < afterLines.length) {
    if (
      left < beforeLines.length
      && right < afterLines.length
      && beforeLines[left] === afterLines[right]
    ) {
      operations.push({ kind: "context", oldLine, newLine, text: beforeLines[left] });
      left += 1;
      right += 1;
      oldLine += 1;
      newLine += 1;
    } else if (
      right < afterLines.length
      && (left === beforeLines.length
        || table[left * width + right + 1] > table[(left + 1) * width + right])
    ) {
      operations.push({ kind: "add", oldLine: null, newLine, text: afterLines[right] });
      right += 1;
      newLine += 1;
    } else {
      operations.push({ kind: "remove", oldLine, newLine: null, text: beforeLines[left] });
      left += 1;
      oldLine += 1;
    }
  }
  return { truncated: false, rows: collapseContext(operations) };
}

function splitLines(value) {
  const lines = value.replace(/\r\n?/g, "\n").split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines;
}

function collapseContext(rows) {
  const changed = rows
    .map((row, index) => row.kind === "context" ? null : index)
    .filter((index) => index !== null);
  if (changed.length === 0) return rows;
  const visible = new Set();
  for (const index of changed) {
    for (
      let cursor = Math.max(0, index - CONTEXT_LINES);
      cursor <= Math.min(rows.length - 1, index + CONTEXT_LINES);
      cursor += 1
    ) {
      visible.add(cursor);
    }
  }
  const result = [];
  let cursor = 0;
  while (cursor < rows.length) {
    if (visible.has(cursor)) {
      result.push(rows[cursor]);
      cursor += 1;
      continue;
    }
    const start = cursor;
    while (cursor < rows.length && !visible.has(cursor)) cursor += 1;
    result.push({ kind: "skip", count: cursor - start });
  }
  return result;
}

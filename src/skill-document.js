import { fromMarkdown } from "mdast-util-from-markdown";

const FRONTMATTER = /^---\s*\r?\n([\s\S]*?)\r?\n---(?:\s*\r?\n|\s*$)/;

function parseFrontmatter(markdown) {
  const match = markdown.match(FRONTMATTER);
  if (!match) return { match: null, name: "", description: "", bodyStart: 0 };

  const values = {};
  const lines = match[1].split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const keyMatch = lines[index].match(/^([A-Za-z0-9_-]+):(?:\s*(.*))?$/);
    if (!keyMatch) continue;
    const [, key, rawValue = ""] = keyMatch;
    if (/^[>|][+-]?$/.test(rawValue.trim())) {
      const folded = [];
      while (index + 1 < lines.length && /^\s+/.test(lines[index + 1])) {
        index += 1;
        folded.push(lines[index].trim());
      }
      values[key] = rawValue.startsWith(">") ? folded.join(" ") : folded.join("\n");
    } else {
      values[key] = rawValue.trim().replace(/^(['"])([\s\S]*)\1$/, "$2");
    }
  }
  return {
    match,
    name: values.name || "",
    description: values.description || "",
    bodyStart: match[0].length
  };
}

function nodeText(node) {
  if (typeof node.value === "string") return node.value;
  return Array.isArray(node.children) ? node.children.map(nodeText).join("") : "";
}

function sectionContent(raw) {
  return raw.replace(/^(?:[ \t]*\r?\n)+/, "").replace(/(?:\r?\n[ \t]*)+$/, "");
}

function contentEnvelope(raw, hasFollowingSection) {
  const leading = raw.match(/^(?:[ \t]*\r?\n)+/)?.[0] || "\n\n";
  const withoutLeading = raw.slice(Math.min(leading.length, raw.length));
  const trailing = withoutLeading.match(/(?:\r?\n[ \t]*)+$/)?.[0] || (hasFollowingSection ? "\n\n" : "\n");
  return { leading, trailing };
}

function analyze(markdown) {
  const frontmatter = parseFrontmatter(markdown);
  const body = markdown.slice(frontmatter.bodyStart);
  const tree = fromMarkdown(body);
  const headings = tree.children.filter((node) => node.type === "heading" && node.position);
  const sections = [];

  const firstHeadingOffset = headings[0]?.position?.start?.offset ?? body.length;
  if (body.slice(0, firstHeadingOffset).trim()) {
    sections.push({
      kind: "preamble",
      level: 0,
      title: "正文开头",
      titleEditable: false,
      headingStart: null,
      headingEnd: null,
      contentStart: 0,
      contentEnd: firstHeadingOffset,
      content: sectionContent(body.slice(0, firstHeadingOffset))
    });
  }

  headings.forEach((heading, headingIndex) => {
    const headingStart = heading.position.start.offset;
    const headingEnd = heading.position.end.offset;
    const contentEnd = headings[headingIndex + 1]?.position?.start?.offset ?? body.length;
    sections.push({
      kind: "heading",
      level: heading.depth,
      title: nodeText(heading),
      titleEditable: true,
      headingStart,
      headingEnd,
      contentStart: headingEnd,
      contentEnd,
      content: sectionContent(body.slice(headingEnd, contentEnd))
    });
  });

  return { frontmatter, body, sections };
}

export function parseSkillDocument(markdown) {
  const analyzed = analyze(markdown);
  return {
    name: analyzed.frontmatter.name,
    description: analyzed.frontmatter.description,
    body: analyzed.body.replace(/^\s+/, ""),
    sections: analyzed.sections.map((section, index) => ({
      index,
      kind: section.kind,
      level: section.level,
      title: section.title,
      titleEditable: section.titleEditable,
      content: section.content
    }))
  };
}

function updateDescription(markdown, description) {
  const { match } = parseFrontmatter(markdown);
  if (!match) return markdown;
  const lines = match[1].split(/\r?\n/);
  const start = lines.findIndex((line) => /^description:/.test(line));
  let end = start;
  if (start >= 0) {
    while (end + 1 < lines.length && /^\s+/.test(lines[end + 1])) end += 1;
  }
  const normalized = description.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  const replacement = ["description: >-", ...(normalized.length ? normalized : [""]).map((line) => `  ${line}`)];
  if (start >= 0) lines.splice(start, end - start + 1, ...replacement);
  else lines.push(...replacement);
  return `---\n${lines.join("\n")}\n---${markdown.slice(match[0].length - (match[0].endsWith("\n") ? 1 : 0))}`;
}

function updateName(markdown, name) {
  const { match } = parseFrontmatter(markdown);
  if (!match) return markdown;
  const lines = match[1].split(/\r?\n/);
  const index = lines.findIndex((line) => /^name:/.test(line));
  if (index >= 0) lines[index] = `name: ${name.trim()}`;
  else lines.unshift(`name: ${name.trim()}`);
  return `---\n${lines.join("\n")}\n---${markdown.slice(match[0].length - (match[0].endsWith("\n") ? 1 : 0))}`;
}

function updateBody(markdown, body) {
  const { match } = parseFrontmatter(markdown);
  if (!match) return body;
  return `${markdown.slice(0, match[0].length).replace(/\s*$/, "")}\n\n${body.replace(/^\s+|\s+$/g, "")}\n`;
}

function updateSection(markdown, mutation) {
  const analyzed = analyze(markdown);
  const section = analyzed.sections[mutation.index];
  if (!section) return markdown;
  const globalOffset = analyzed.frontmatter.bodyStart;

  if (mutation.type === "section-title") {
    const title = mutation.value.trim();
    if (!section.titleEditable || !title) return markdown;
    const replacement = `${"#".repeat(section.level)} ${title}`;
    return `${markdown.slice(0, globalOffset + section.headingStart)}${replacement}${markdown.slice(globalOffset + section.headingEnd)}`;
  }

  const raw = analyzed.body.slice(section.contentStart, section.contentEnd);
  const envelope = contentEnvelope(raw, mutation.index < analyzed.sections.length - 1);
  const value = mutation.value.replace(/^\s+|\s+$/g, "");
  const replacement = value ? `${envelope.leading}${value}${envelope.trailing}` : envelope.leading;
  return `${markdown.slice(0, globalOffset + section.contentStart)}${replacement}${markdown.slice(globalOffset + section.contentEnd)}`;
}

export function updateSkillDocument(markdown, mutation) {
  switch (mutation.type) {
    case "name":
      return updateName(markdown, mutation.value);
    case "description":
      return updateDescription(markdown, mutation.value);
    case "body":
      return updateBody(markdown, mutation.value);
    case "section-title":
    case "section-content":
      return updateSection(markdown, mutation);
    default:
      return markdown;
  }
}

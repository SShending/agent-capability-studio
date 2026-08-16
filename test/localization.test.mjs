import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  catalogs,
  catalogParity,
  createLocalization,
  DEFAULT_LOCALE,
  LOCALE_STORAGE_KEY,
  normalizeLocale
} from "../src/localization.js";

test("localization catalogs have identical stable keys", () => {
  for (const result of Object.values(catalogParity())) {
    assert.deepEqual(result, { missing: [], extra: [] });
  }
});

test("Simplified Chinese UI does not expose untranslated domain terms", () => {
  const untranslatedDomainTerm = /\b(?:Skills?|Packages?|Bundles?|Collections?)\b/;
  const offenders = Object.entries(catalogs["zh-CN"])
    .filter(([, value]) => untranslatedDomainTerm.test(value))
    .map(([key, value]) => `${key}: ${value}`);

  assert.deepEqual(offenders, []);
});

test("static Chinese fallback copy does not expose untranslated domain terms", async () => {
  const html = await readFile(new URL("../index.html", import.meta.url), "utf8");
  const untranslatedDomainTerm = /\b(?:Skills?|Packages?|Bundles?|Collections?)\b/;
  const allowedProductNames = new Set(["Agent Skill Studio", "Skill Studio"]);
  const visibleText = [...html.matchAll(/>([^<>]+)</g)].map((match) => match[1].trim());
  const accessibleText = [...html.matchAll(/\s(?:aria-label|title)="([^"]+)"/g)]
    .map((match) => match[1].trim());
  const offenders = [...visibleText, ...accessibleText]
    .filter(Boolean)
    .filter((value) => !allowedProductNames.has(value))
    .filter((value) => untranslatedDomainTerm.test(value));

  assert.deepEqual(offenders, []);
});

test("localization persists an explicit language and interpolates values", () => {
  const values = new Map();
  const storage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value)
  };
  const localization = createLocalization({ storage });
  assert.equal(localization.locale, DEFAULT_LOCALE);
  assert.equal(localization.t("list.results", { visible: 2, total: 8 }), "2 个结果 · 8 个已收录");
  assert.equal(localization.setLocale("en"), true);
  assert.equal(values.get(LOCALE_STORAGE_KEY), "en");
  assert.equal(localization.t("list.results", { visible: 2, total: 8 }), "2 results · 8 in library");
});

test("unsupported locale values cannot escape the supported catalog", () => {
  assert.equal(normalizeLocale("en"), "en");
  assert.equal(normalizeLocale("fr"), DEFAULT_LOCALE);
  assert.equal(normalizeLocale(null), DEFAULT_LOCALE);
});

test("every static marker and literal translation call has a catalog entry", async () => {
  const [html, app] = await Promise.all([
    readFile(new URL("../index.html", import.meta.url), "utf8"),
    readFile(new URL("../src/app.js", import.meta.url), "utf8")
  ]);
  const used = new Set();
  for (const match of html.matchAll(/data-i18n(?:-(?:aria-label|title|placeholder))?="([^"]+)"/g)) {
    used.add(match[1]);
  }
  for (const match of app.matchAll(/\bt\("([^"]+)"/g)) used.add(match[1]);
  const missing = [...used].filter((key) => !(key in catalogs[DEFAULT_LOCALE])).sort();
  assert.deepEqual(missing, []);
});

test("each source catalog declares every key only once", async () => {
  const source = await readFile(new URL("../src/localization.js", import.meta.url), "utf8");
  const zhSource = source.match(/const zhCN = \{([\s\S]*?)\n\};\n\nconst en =/)[1];
  const enSource = source.match(/const en = \{([\s\S]*?)\n\};\n\nexport const catalogs/)[1];
  for (const catalogSource of [zhSource, enSource]) {
    const keys = [...catalogSource.matchAll(/^\s*"([^"]+)":/gm)].map((match) => match[1]);
    const duplicates = keys.filter((key, index) => keys.indexOf(key) !== index);
    assert.deepEqual([...new Set(duplicates)].sort(), []);
  }
});

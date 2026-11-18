import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { countdown } from "../src/format.js";

const languages = ["ca", "es", "en", "fr", "de", "it"];
const dictionaries = Object.fromEntries(languages.map((locale) => [
  locale, JSON.parse(readFileSync(new URL(`../locales/${locale}.json`, import.meta.url), "utf8")),
]));
test("all six languages cover the same keys and interpolation arguments", () => {
  for (const locale of languages) {
    assert.deepEqual(Object.keys(dictionaries[locale]).sort(), Object.keys(dictionaries.en).sort());
    for (const key of Object.keys(dictionaries.en)) {
      assert.ok(dictionaries[locale][key].trim(), `${locale}: ${key}`);
      assert.deepEqual(
        [...dictionaries[locale][key].matchAll(/\{\w+\}/g)].map(([match]) => match).sort(),
        [...dictionaries.en[key].matchAll(/\{\w+\}/g)].map(([match]) => match).sort(),
        `${locale}: ${key}`,
      );
      assert.ok(!dictionaries[locale][key].includes("Ciclo Codex"));
    }
  }
});
test("unknown and due reset times use the selected language", () => {
  for (const locale of languages) {
    const translate = (key) => dictionaries[locale][key];
    assert.equal(countdown(0, 100, translate, locale), dictionaries[locale].resetUnknown);
    assert.equal(countdown(99, 100, translate, locale), dictionaries[locale].resetDue);
    assert.ok(countdown(3700, 100, translate, locale).includes("1"));
  }
});

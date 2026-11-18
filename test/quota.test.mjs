import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { quotaMarkup, usageColor } from "../src/quota.js";

const languages = ["ca", "es", "en", "fr", "de", "it"];
const dictionaries = Object.fromEntries(languages.map((locale) => [
  locale, JSON.parse(readFileSync(new URL(`../locales/${locale}.json`, import.meta.url), "utf8")),
]));
const options = (locale = "ca") => ({
  locale,
  resetTime: () => "2 h",
  translate: (key, params = {}) => dictionaries[locale][key].replace(/\{(\w+)\}/g, (_, name) => params[name]),
});

test("panel percentages and bars both show used quota, without quota rings", () => {
  const usage = { windows: [
    { used: 33, minutes: 300, resetsAt: 1000 },
    { used: 35, minutes: 10080, resetsAt: 2000 },
  ] };
  for (const locale of languages) {
    const markup = quotaMarkup(usage, options(locale));
    assert.ok(markup.includes("33<small>%</small>"));
    assert.ok(markup.includes("35<small>%</small>"));
    assert.ok(markup.includes('aria-valuenow="33"'));
    assert.ok(markup.includes('aria-valuenow="35"'));
    assert.ok(markup.includes("width:33%"));
    assert.ok(markup.includes("width:35%"));
    assert.ok(!markup.includes("width:67%"));
    assert.ok(!markup.includes("width:65%"));
    assert.ok(markup.includes(dictionaries[locale].usedQuota));
    assert.ok(markup.includes(dictionaries[locale].usageCaption));
    assert.ok(!markup.includes("<svg"));
    assert.ok(!markup.includes("<circle"));
  }
});
test("unknown quota is not rendered as zero", () => {
  assert.ok(!quotaMarkup(null, options()).includes('role="progressbar"'));
  assert.ok(!quotaMarkup({ windows: [] }, options()).includes('role="progressbar"'));
});
test("used-quota warnings increase as usage rises", () => {
  assert.equal(usageColor(74), "var(--mint)");
  assert.equal(usageColor(75), "var(--amber)");
  assert.equal(usageColor(90), "var(--danger)");
});
test("bar values are clamped and external display text is escaped", () => {
  const opts = { ...options(), resetTime: () => '<img src=x onerror="bad()">' };
  const markup = quotaMarkup({ windows: [
    { used: -12, minutes: 300, resetsAt: 1000 },
    { used: 120, minutes: 10080, resetsAt: 1000 },
  ] }, opts);
  assert.ok(markup.includes("width:0%"));
  assert.ok(markup.includes("width:100%"));
  assert.ok(!markup.includes("<img"));
});

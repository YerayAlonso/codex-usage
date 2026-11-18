import test from "node:test";
import assert from "node:assert/strict";
import { countdown, remaining, escapeHtml, stale } from "../src/format.js";

test("remaining quota is clamped", () => {
  assert.equal(remaining(30), 70);
  assert.equal(remaining(150), 0);
  assert.equal(remaining(-10), 100);
});

test("unknown reset is not an imminent reset", () => {
  assert.equal(countdown(0, 100), "Hora desconeguda");
  assert.equal(countdown(100, 200), "Pendent d’actualitzar");
  assert.equal(countdown(100 + 90000, 100), "1 d 1 h");
});

test("expired quota and failed reads are marked stale", () => {
  const usage = { updatedAt: 100, windows: [{ resetsAt: 500 }] };
  assert.equal(stale(usage, null, 200), false);
  assert.equal(stale(usage, "error", 200), true);
  assert.equal(stale(usage, null, 500), true);
  assert.equal(stale(null, null, 200), true);
});

test("CLI output cannot inject markup into activity", () => {
  assert.equal(escapeHtml('<script>"&'), "&lt;script&gt;&quot;&amp;");
});

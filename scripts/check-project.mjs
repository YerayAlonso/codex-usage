import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(resolve(root, path), "utf8");
const json = (path) => JSON.parse(read(path));
const pkg = json("package.json");
const lock = json("package-lock.json");
const config = json("src-tauri/tauri.conf.json");
const cargo = read("src-tauri/Cargo.toml");
const cargoPackage = cargo.split("[package]")[1]?.split(/\n\[/)[0] ?? "";
const cargoVersion = cargoPackage.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const cargoLockPackage = read("src-tauri/Cargo.lock").split("[[package]]")
  .find((entry) => /^\s*name = "codex-usage"$/m.test(entry));
const repository = "https://github.com/YerayAlonso/codex-usage";

assert.equal(pkg.name, "codex-usage", "Unexpected package name");
assert.equal(pkg.private, true, "This app must not accidentally publish to npm");
assert.equal(pkg.license, "MIT");
assert.equal(pkg.author, "YerayAlonso");
assert.equal(pkg.repository.url, `git+${repository}.git`);
assert.equal(pkg.bugs.url, `${repository}/issues`);
assert.equal(pkg.version, lock.version, "npm lockfile version mismatch");
assert.equal(pkg.version, lock.packages[""].version, "npm root lockfile version mismatch");
assert.equal(pkg.version, cargoVersion, "Cargo package version mismatch");
assert.equal(pkg.version, config.version, "Tauri version mismatch");
assert.ok(cargoLockPackage?.includes(`version = "${pkg.version}"`), "Cargo lockfile version mismatch");
assert.match(cargoPackage, /^license\s*=\s*"MIT"$/m);
assert.match(cargoPackage, /^publish\s*=\s*false$/m);
assert.ok(cargoPackage.includes(`repository = "${repository}"`));
assert.equal(config.productName, "Codex Usage");
assert.equal(config.identifier, "dev.codex.usage", "Changing this moves users' app data");
assert.equal(config.bundle.publisher, "YerayAlonso");
assert.equal(config.bundle.homepage, repository);
assert.equal(config.bundle.license, "MIT");
assert.equal(config.bundle.licenseFile, "../LICENSE");

for (const path of [
  "LICENSE", "README.md", "docs/README.ca.md", "CONTRIBUTING.md", "SECURITY.md",
  "package-lock.json", "src-tauri/Cargo.lock", ".github/workflows/ci.yml",
  ".github/workflows/build-packages.yml",
]) {
  assert.ok(existsSync(resolve(root, path)), `Missing ${path}`);
}
for (const path of config.bundle.icon) {
  assert.ok(existsSync(resolve(root, "src-tauri", path)), `Missing bundle icon: ${path}`);
}
assert.match(read("LICENSE"), /^MIT License\n/);
assert.ok(read("LICENSE").includes("Copyright (c) 2026 YerayAlonso"));

console.log(`Project metadata OK: YerayAlonso/codex-usage v${pkg.version} · MIT`);

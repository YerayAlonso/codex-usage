import * as esbuild from "esbuild";
import { copyFileSync, mkdirSync, rmSync } from "node:fs";

const dev = process.argv.includes("--dev");
// This directory contains only generated assets; never ship stale development maps.
rmSync("dist", { recursive: true, force: true });
mkdirSync("dist", { recursive: true });
copyFileSync("index.html", "dist/index.html");
const options = {
  entryPoints: ["src/main.ts"],
  bundle: true,
  outdir: "dist",
  format: "esm",
  target: "es2022",
  minify: !dev,
  sourcemap: dev,
  logLevel: "info",
};
if (dev) {
  copyFileSync("test/layout.browser.mjs", "dist/layout.browser.mjs");
  const context = await esbuild.context(options);
  await context.watch();
  await context.serve({ servedir: "dist", host: "127.0.0.1", port: 1420 });
  console.log("Frontend: http://127.0.0.1:1420 (reobre el panell per carregar els canvis)");
} else {
  await esbuild.build(options);
}

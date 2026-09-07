#!/usr/bin/env node
import { execSync } from "node:child_process";
// Set the project version everywhere it is declared:
//   package.json, package-lock.json, src-tauri/tauri.conf.json, Cargo.toml ([workspace.package]).
// Usage: npm run version:bump 0.2.0   (or: node scripts/bump-version.mjs 0.2.0)
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error("usage: bump-version.mjs <semver>   e.g. 0.2.0 or 0.2.0-beta.1");
  process.exit(2);
}

const json = (path, edit) => {
  const data = JSON.parse(readFileSync(path, "utf8"));
  edit(data);
  writeFileSync(path, `${JSON.stringify(data, null, 2)}\n`);
  console.log(`updated ${path}`);
};

json("package.json", (d) => {
  d.version = version;
});
json("package-lock.json", (d) => {
  d.version = version;
  if (d.packages?.[""]) d.packages[""].version = version;
});
json("src-tauri/tauri.conf.json", (d) => {
  d.version = version;
});

const cargo = readFileSync("Cargo.toml", "utf8");
const updated = cargo.replace(/(\[workspace\.package\][\s\S]*?version\s*=\s*")[^"]+(")/, `$1${version}$2`);
if (updated === cargo) {
  console.error("could not find [workspace.package] version in Cargo.toml");
  process.exit(1);
}
writeFileSync("Cargo.toml", updated);
console.log("updated Cargo.toml");

// Refresh Cargo.lock so the workspace crates carry the new version.
try {
  execSync("cargo update -w --offline", { stdio: "inherit" });
} catch {
  console.warn("cargo update -w failed (offline?); run it manually before committing");
}

console.log(`\nversion is now ${version}. Next:\n  git commit -am "Release v${version}"\n  git tag v${version}\n  git push origin HEAD v${version}`);

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const readJson = (path) => JSON.parse(readFileSync(join(root, path), "utf8"));
const lock = readJson("package-lock.json");
const versions = {
  tauri: readJson("src-tauri/tauri.conf.json").version,
  npm: readJson("package.json").version,
  lockfile: lock.version,
  lockfileRoot: lock.packages?.[""]?.version,
  cargo: readFileSync(join(root, "Cargo.toml"), "utf8").match(/^version\s*=\s*"([^"]+)"/m)?.[1],
};

if (Object.values(versions).some((version) => !version || version !== versions.tauri)) {
  console.error("Package versions disagree:", versions);
  process.exit(1);
}
console.log(`All package versions agree: ${versions.tauri}`);

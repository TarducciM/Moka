// La versione vive in tre posti (package.json, Cargo.toml, tauri.conf.json) più
// i due lock (package-lock.json, Cargo.lock): devono dire tutti la stessa
// cosa. Una versione disallineata è una versione bugiarda, e l'updater (dalla
// 0.3) confronta proprio quella.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(join(root, p), "utf8");

const versions = {
  "package.json": JSON.parse(read("package.json")).version,
  "package-lock.json": JSON.parse(read("package-lock.json")).version,
  "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json")).version,
  "src-tauri/Cargo.toml": read("src-tauri/Cargo.toml").match(/^version\s*=\s*"([^"]+)"/m)?.[1],
  "src-tauri/Cargo.lock": read("src-tauri/Cargo.lock").match(
    /\[\[package\]\]\r?\nname = "moka"\r?\nversion = "([^"]+)"/,
  )?.[1],
};

const distinct = new Set(Object.values(versions));
if (distinct.size !== 1 || distinct.has(undefined)) {
  console.error("Versioni disallineate:");
  for (const [file, v] of Object.entries(versions)) console.error(`  ${file}: ${v}`);
  process.exit(1);
}
console.log(`Versione ${[...distinct][0]} ovunque.`);

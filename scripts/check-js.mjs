// `node --check` su ogni file JavaScript del repo: senza bundler, un errore di
// sintassi si scoprirebbe solo aprendo la finestra.

import { execFileSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const files = ["src", "scripts", "tests", "site"].flatMap((dir) =>
  readdirSync(join(root, dir))
    .filter((f) => f.endsWith(".js") || f.endsWith(".mjs"))
    .map((f) => join(dir, f)),
);

for (const file of files) {
  execFileSync(process.execPath, ["--check", join(root, file)], { stdio: "inherit" });
}
console.log(`Sintassi ok: ${files.length} file.`);

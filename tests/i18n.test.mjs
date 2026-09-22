// Le traduzioni non falliscono mai da sole: una chiave mancante mostra la
// chiave stessa a schermo, e una frase presente solo in italiano arriva in
// italiano a chi usa l'inglese (sembra funzionare, ed è peggio). Questo test è
// l'unico posto in cui un errore del genere diventa rosso.
//
// Controlla, per tutte le lingue in src/locales/:
// - stesse chiavi, nessun valore vuoto, stessi segnaposto {nome};
// - ogni chiave usata in HTML (data-i18n*), JS e Rust esiste;
// - ogni chiave del dizionario è usata da qualche parte.

import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const localesDir = join(root, "src", "locales");
const locales = Object.fromEntries(
  readdirSync(localesDir)
    .filter((f) => f.endsWith(".json"))
    .map((f) => [f.replace(".json", ""), JSON.parse(readFileSync(join(localesDir, f), "utf8"))]),
);
const reference = locales.it;

const read = (dir, ext) =>
  readdirSync(dir)
    .filter((f) => f.endsWith(ext))
    .map((f) => ({ file: f, text: readFileSync(join(dir, f), "utf8") }));

const KEY = /^(state|time|mode|reason|menu|popover|settings|lid|notify)\.[a-z0-9_]+$/;
const placeholders = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

function usedKeys() {
  const used = new Map();
  // "settings.json" ha la forma di una chiave ma è un nome di file.
  const FILE = /\.(json|html|js|mjs|css|svg|png)$/;
  const add = (key, where) => {
    if (KEY.test(key) && !FILE.test(key)) used.set(key, where);
  };
  for (const { file, text } of read(join(root, "src"), ".html")) {
    for (const m of text.matchAll(/data-i18n(?:-title|-aria)?="([^"]+)"/g)) add(m[1], file);
  }
  for (const { file, text } of read(join(root, "src"), ".js")) {
    for (const m of text.matchAll(/["'`]([a-z]+\.[a-z0-9_]+)["'`]/g)) add(m[1], file);
  }
  for (const { file, text } of read(join(root, "src-tauri", "src"), ".rs")) {
    for (const m of text.matchAll(/"([a-z]+\.[a-z0-9_]+)"/g)) add(m[1], file);
  }
  return used;
}

test("ogni lingua ha le stesse chiavi dell'italiano", () => {
  for (const [lang, dict] of Object.entries(locales)) {
    assert.deepEqual(Object.keys(dict).sort(), Object.keys(reference).sort(), lang);
  }
});

test("nessun testo vuoto", () => {
  for (const [lang, dict] of Object.entries(locales)) {
    for (const [key, value] of Object.entries(dict)) {
      assert.ok(typeof value === "string" && value.trim() !== "", `${lang}: ${key} è vuota`);
    }
  }
});

test("gli stessi segnaposto in ogni lingua", () => {
  for (const [lang, dict] of Object.entries(locales)) {
    for (const key of Object.keys(reference)) {
      assert.deepEqual(placeholders(dict[key]), placeholders(reference[key]), `${lang}: ${key}`);
    }
  }
});

test("ogni chiave usata esiste", () => {
  for (const [key, where] of usedKeys()) {
    assert.ok(key in reference, `"${key}" usata in ${where} ma assente da locales/it.json`);
  }
});

test("ogni chiave del dizionario è usata", () => {
  const used = usedKeys();
  const unused = Object.keys(reference).filter((k) => !used.has(k));
  assert.deepEqual(unused, [], `chiavi mai usate: ${unused.join(", ")}`);
});

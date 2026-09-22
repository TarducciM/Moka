// Genera le icone di Moka dai disegni SVG scritti a mano in questo file.
//
//   npm run icons
//
// Scrive:
//   assets/app-icon.svg, src/app-icon.svg icona dell'app (installer, Impostazioni)
//   assets/tray/<stato>-<barra>.svg        sorgenti dell'icona nella tray
//   src-tauri/icons/                       icone dell'app in tutti i formati
//   src-tauri/icons/tray/<stato>-<barra>-<px>.png
//
// <stato>: off (contorno), system (piena), display (piena, con vapore).
// <barra>: dark = per la barra delle applicazioni scura (disegno bianco),
//          light = per la barra chiara (disegno quasi nero).
//
// La rasterizzazione la fa `tauri icon` (resvg): nessuna dipendenza in più.
// Le PNG si committano: la build non ha bisogno di rigenerarle.

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const SIZES = [16, 20, 24, 32, 40, 48];
const INK = { dark: "#ffffff", light: "#1a1a1a" };
const COFFEE = "#8b5a2b";
const CREAM = "#f6ede3";

// La moka, su una griglia 32×32: corpo a clessidra con il beccuccio a sinistra,
// pomello sul coperchio, manico a destra, due fili di vapore sopra.
const BODY = "M6.5 10 L22 10 L20 17 L23 28 L9 28 L12 17 L10.5 13.5 Z";
const KNOB = { x: 14, y: 7, w: 4, h: 3 };
const HANDLE = "M21.6 11.5 H26 L25 16 H20.3";
const STEAM = "M12.5 6 q-1.4 -1.1 0 -2.2 t0 -2.2 M19.5 6 q-1.4 -1.1 0 -2.2 t0 -2.2";

function pot({ color, filled, steam, strokeWidth = 2 }) {
  const knob = `<rect x="${KNOB.x}" y="${KNOB.y}" width="${KNOB.w}" height="${KNOB.h}" rx="0.6" fill="${color}"/>`;
  return [
    `<path d="${BODY}" fill="${filled ? color : "none"}" stroke="${color}" stroke-width="${strokeWidth}" stroke-linejoin="round"/>`,
    knob,
    `<path d="${HANDLE}" fill="none" stroke="${color}" stroke-width="${strokeWidth}" stroke-linejoin="round" stroke-linecap="round"/>`,
    steam
      ? `<path d="${STEAM}" fill="none" stroke="${color}" stroke-width="1.75" stroke-linecap="round"/>`
      : "",
  ].join("\n  ");
}

function traySvg(state, bar) {
  const color = INK[bar];
  const body = pot({ color, filled: state !== "off", steam: state === "display" });
  // viewBox stretto attorno al disegno (vapore compreso), uguale per i tre
  // stati: l'icona non deve "saltare" quando cambia stato.
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="2 0.5 29 29">\n  ${body}\n</svg>\n`;
}

// A 16 px (scala 100%) il disegno vettoriale si impasta: il vapore diventa due
// macchie e il foro del manico sparisce. Per quella dimensione la moka è
// disegnata pixel per pixel. `#` = pieno; la versione a contorno si ricava
// togliendo i pixel interni.
const PIXELS_16 = [
  "................", // 0  vapore
  "................", // 1
  "................", // 2
  ".......##.......", // 3  pomello
  "..##########....", // 4  bordo superiore e beccuccio
  "....##########..", // 5  attacco del manico
  ".....######..#..", // 6
  ".....######..#..", // 7
  "......########..", // 8  vita e attacco basso del manico
  "......####......", // 9
  ".....######.....", // 10
  ".....######.....", // 11
  "....########....", // 12
  "....########....", // 13
  "...##########...", // 14 base
  "................", // 15
];
const STEAM_16 = [
  [5, 0], [4, 1], [5, 2],
  [10, 0], [9, 1], [10, 2],
];
// Pixel che restano pieni anche nella versione a contorno (pomello, manico).
const SOLID_16 = (x, y) => y === 3 || x >= 12 || (y === 8 && x >= 10);

function pixelSvg(state, bar) {
  const color = INK[bar];
  const on = (x, y) => PIXELS_16[y]?.[x] === "#";
  const cells = [];
  for (let y = 0; y < 16; y++) {
    for (let x = 0; x < 16; x++) {
      if (!on(x, y)) continue;
      const inner = on(x - 1, y) && on(x + 1, y) && on(x, y - 1) && on(x, y + 1);
      if (state === "off" && inner && !SOLID_16(x, y)) continue;
      cells.push([x, y]);
    }
  }
  if (state === "display") cells.push(...STEAM_16);
  const rects = cells
    .map(([x, y]) => `<rect x="${x}" y="${y}" width="1" height="1"/>`)
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" shape-rendering="crispEdges" fill="${color}">${rects}</svg>\n`;
}

function appSvg() {
  const body = pot({ color: CREAM, filled: true, steam: true });
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <rect x="48" y="48" width="928" height="928" rx="208" fill="${COFFEE}"/>
  <g transform="translate(512 520) scale(23) translate(-16.25 -15)">
  ${body}
  </g>
</svg>
`;
}

function tauriIcon(args) {
  // Il CLI si lancia con node, senza shell: niente quoting da gestire.
  const cli = join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
  execFileSync(process.execPath, [cli, "icon", ...args], {
    cwd: root,
    stdio: ["ignore", "ignore", "inherit"],
  });
}

mkdirSync(join(root, "assets", "tray"), { recursive: true });
mkdirSync(join(root, "src-tauri", "icons", "tray"), { recursive: true });

writeFileSync(join(root, "assets", "app-icon.svg"), appSvg());
// La stessa, servita alle Impostazioni.
writeFileSync(join(root, "src", "app-icon.svg"), appSvg());
tauriIcon([join(root, "assets", "app-icon.svg"), "-o", join(root, "src-tauri", "icons")]);
// Moka è solo per Windows: le icone per Android e iOS non servono.
for (const extra of ["android", "ios"]) {
  rmSync(join(root, "src-tauri", "icons", extra), { recursive: true, force: true });
}

const scratch = mkdtempSync(join(tmpdir(), "moka-icons-"));
try {
  for (const state of ["off", "system", "display"]) {
    for (const bar of ["dark", "light"]) {
      const vector = join(root, "assets", "tray", `${state}-${bar}.svg`);
      const pixel = join(root, "assets", "tray", `${state}-${bar}-16.svg`);
      writeFileSync(vector, traySvg(state, bar));
      writeFileSync(pixel, pixelSvg(state, bar));
      const out = join(scratch, `${state}-${bar}`);
      const outPixel = join(scratch, `${state}-${bar}-16`);
      tauriIcon([vector, "-o", out, "-p", SIZES.filter((s) => s !== 16).join(",")]);
      tauriIcon([pixel, "-o", outPixel, "-p", "16"]);
      for (const size of SIZES) {
        copyFileSync(
          join(size === 16 ? outPixel : out, `${size}x${size}.png`),
          join(root, "src-tauri", "icons", "tray", `${state}-${bar}-${size}.png`),
        );
      }
    }
  }
} finally {
  rmSync(scratch, { recursive: true, force: true });
}

console.log("Icone generate.");

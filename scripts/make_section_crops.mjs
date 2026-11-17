// Cut a form page into its own sections and emit side-by-side review crops.
//
// WHY. Reading a whole-page percentage tells you a form is wrong without
// telling you where, and a whole-page diff at 144 DPI is too dense to read.
// Structural defects are obvious to a human eye at section scale - the 2551Q
// footnote rendering at 7pt against an official 8.04pt was invisible in every
// metric for a whole session and took one look at a cropped strip to find.
//
// Sections are cut at the FORM'S OWN horizontal rules (from the tracked
// geometry contract), not at arbitrary intervals, so each crop is a band the
// form itself defines: a part heading, a table block, a signature area.
//
// Each output is official-on-top / ours-below with a separator, which is the
// arrangement that makes a weight or spacing difference jump out. Diffing is
// left to the eye deliberately: a red-overlay diff hides WHICH side is wrong.
//
// Usage:
//   node scripts/make_section_crops.mjs --form 1601C --page 1 [--out DIR]
//   node scripts/make_section_crops.mjs --form 1601C --page 1 --min-band 40

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { PNG } from "pngjs";

const REPO = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");

function arg(name, fallback = null) {
  const index = process.argv.indexOf(`--${name}`);
  return index === -1 ? fallback : process.argv[index + 1];
}

const FORM = (arg("form") ?? "1601C").toUpperCase();
const PAGE = Number(arg("page") ?? 1);
const MIN_BAND_PX = Number(arg("min-band") ?? 56);
const OUT_DIR = arg("out") ?? path.join(REPO, "test-results/section-crops", `${FORM.toLowerCase()}-p${PAGE}`);

// reference stem -> the naming the references directory uses
const STEMS = {
  "0605": "0605-1999", "0619E": "0619e-2018", "0619F": "0619f-2018",
  "1601C": "1601c-2018", "1701": "1701-2018", "1701Q": "1701q-2018",
  "1702MX": "1702mx-2018c", "1702RT": "1702rt-2018c",
  "2550Q": "2550q-2024", "2551Q": "2551q-2018"
};

const stem = STEMS[FORM];
if (!stem) {
  console.error(`unknown form ${FORM}; known: ${Object.keys(STEMS).join(", ")}`);
  process.exit(2);
}

const referencePath = path.join(REPO, `packages/form-renderer/references/${stem}-page-${PAGE}-chromium.png`);

// The captured render lands under a hashed Playwright directory; find it by the
// artifact name rather than guessing the hash.
function findActual() {
  const root = path.join(REPO, "test-results/form-renderer");
  if (!fs.existsSync(root)) return null;
  const wanted = [
    `${stem}-page-${PAGE}-actual.png`,
    `${FORM.toLowerCase()}-page-${PAGE}-actual.png`
  ];
  for (const dir of fs.readdirSync(root)) {
    for (const name of wanted) {
      const candidate = path.join(root, dir, name);
      if (fs.existsSync(candidate)) return candidate;
    }
  }
  return null;
}

const actualPath = findActual();
if (!fs.existsSync(referencePath)) {
  console.error(`missing reference: ${path.relative(REPO, referencePath)}`);
  process.exit(2);
}
if (!actualPath) {
  console.error(
    `no captured render for ${FORM} page ${PAGE}. Run the visual suite first:\n` +
    `  FORM_VISUAL_NON_PROMOTING_ALLOW_DIRTY_SOURCE=1 npm run test:forms:visual`
  );
  process.exit(2);
}

const reference = PNG.sync.read(fs.readFileSync(referencePath));
const actual = PNG.sync.read(fs.readFileSync(actualPath));
if (reference.width !== actual.width) {
  console.error(`width mismatch: reference ${reference.width}, actual ${actual.width}`);
  process.exit(2);
}

// Section boundaries come from the form's own full-width horizontal rules.
// Falling back to fixed bands would cut through tables and hide exactly the
// misalignments this tool exists to surface.
function boundariesFromContract() {
  const contractPath = path.join(REPO, `packages/form-specs/geometry-contracts/${stem}.json`);
  if (!fs.existsSync(contractPath)) return null;
  const contract = JSON.parse(fs.readFileSync(contractPath, "utf8"));
  const page = contract.pages?.[PAGE - 1];
  if (!page) return null;
  const [widthPt] = page.size_pt ?? [612];
  const wide = (page.rules_h ?? [])
    .filter((rule) => {
      // Only rules spanning most of the page delimit a section; a table's
      // interior rules would shred the page into unreadable slivers.
      if ((rule.length_pt ?? 0) < widthPt * 0.55) return false;
      // Cut only on BLACK ink. Light-grey band fills (gray ~= 0.8509, tone
      // ~217) are decoration, not section boundaries - the same distinction
      // that refuted seven false "missing rule" findings this wave.
      return (rule.gray ?? 1) < 0.5;
    })
    .map((rule) => Math.round(rule.pos_px ?? 0))
    .filter((y) => y > 0 && y < reference.height);
  return [...new Set(wide)].sort((a, b) => a - b);
}

let cuts = boundariesFromContract();
let cutSource = "geometry contract full-width rules";
if (!cuts || cuts.length < 2) {
  const step = 240;
  cuts = [];
  for (let y = step; y < reference.height; y += step) cuts.push(y);
  cutSource = "fixed 240px fallback (no contract rules found)";
}

// Merge cuts closer than MIN_BAND_PX so a stack of adjacent rules produces one
// readable band rather than several slivers.
const merged = [];
for (const cut of cuts) {
  if (merged.length === 0 || cut - merged[merged.length - 1] >= MIN_BAND_PX) merged.push(cut);
}
const bounds = [0, ...merged, reference.height];

fs.mkdirSync(OUT_DIR, { recursive: true });
const GAP = 10;
const LABEL = 0;
const manifest = [];

for (let index = 0; index < bounds.length - 1; index += 1) {
  const top = bounds[index];
  const bottom = bounds[index + 1];
  const height = bottom - top;
  if (height < 12) continue;

  const width = reference.width;
  const out = new PNG({ width, height: height * 2 + GAP + LABEL });
  out.data.fill(255);

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const src = ((top + y) * width + x) * 4;
      const topDst = (y * width + x) * 4;
      const bottomDst = ((height + GAP + y) * width + x) * 4;
      for (let channel = 0; channel < 4; channel += 1) {
        out.data[topDst + channel] = reference.data[src + channel];
        out.data[bottomDst + channel] = actual.data[src + channel];
      }
    }
  }
  // Grey separator so the eye never confuses the two halves.
  for (let y = height; y < height + GAP; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const dst = (y * width + x) * 4;
      out.data[dst] = 120; out.data[dst + 1] = 120; out.data[dst + 2] = 200; out.data[dst + 3] = 255;
    }
  }

  const name = `band-${String(index).padStart(2, "0")}-y${top}-${bottom}.png`;
  fs.writeFileSync(path.join(OUT_DIR, name), PNG.sync.write(out));
  manifest.push({ band: index, y_top: top, y_bottom: bottom, height, file: name });
}

fs.writeFileSync(
  path.join(OUT_DIR, "manifest.json"),
  `${JSON.stringify({
    form: FORM,
    page: PAGE,
    purpose: "non_promotional_visual_section_review",
    promotion_eligible: false,
    layout: "official reference on TOP, our render BELOW, blue separator between",
    cut_source: cutSource,
    reference: path.relative(REPO, referencePath),
    actual: path.relative(REPO, actualPath),
    bands: manifest
  }, null, 2)}\n`
);

console.log(`${FORM} page ${PAGE}: ${manifest.length} bands (cuts from ${cutSource})`);
console.log(`  ${path.relative(REPO, OUT_DIR)}`);
for (const band of manifest) {
  console.log(`  ${band.file}  y ${band.y_top}..${band.y_bottom} (${band.height}px)`);
}

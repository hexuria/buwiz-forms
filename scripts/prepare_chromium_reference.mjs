#!/usr/bin/env node
// Pin and render an official BIR PDF into calibration-only Chromium-raster
// PNG references. The official PDF is converted per page to vector SVG with
// pdftocairo and rasterized by the same Chromium/Playwright environment that
// captures the parity screenshots, eliminating cross-rasterizer noise from
// the visual gate. The Poppler raster prepared by
// scripts/reference/prepare_official_reference.py
// remains the pinned secondary diagnostic; this script computes and records
// the per-page noise floor between the two rasters.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

import { chromium } from "playwright";
import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";

const FORM_CODE_RE = /^[A-Za-z0-9]+$/;
const REVISION_RE = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;
const SHA256_RE = /^[0-9a-f]{64}$/;
const PIXELMATCH_THRESHOLD = 0.1;
const VIEWPORT = { width: 900, height: 1400 };

function fail(message) {
  throw new Error(message);
}

function sha256File(filePath) {
  return createHash("sha256").update(readFileSync(filePath)).digest("hex");
}

function identity(formCode, revision) {
  const code = formCode.trim().toUpperCase();
  const rev = revision.trim();
  if (!FORM_CODE_RE.test(code)) {
    fail("form code must contain only letters and digits");
  }
  if (!REVISION_RE.test(rev)) {
    fail("revision contains unsupported characters");
  }
  return { code, revision: rev, slug: code.toLowerCase(), formId: `${code}v${rev}` };
}

function requireTool(name) {
  const probe = spawnSync(name, ["-v"], { encoding: "utf8" });
  if (probe.error) {
    fail(`required Poppler tool is missing: ${name}`);
  }
  const banner = `${probe.stdout}\n${probe.stderr}`;
  const version = banner.match(/version\s+([0-9][0-9.]*)/);
  if (!version) {
    fail(`cannot determine ${name} version`);
  }
  return version[1];
}

function pngSize(filePath) {
  const header = readFileSync(filePath).subarray(0, 24);
  if (
    header.length !== 24 ||
    !header.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]))
  ) {
    fail(`not a valid PNG: ${filePath}`);
  }
  return { width: header.readUInt32BE(16), height: header.readUInt32BE(24 - 4) };
}

function stableStringify(value, indent = 0) {
  const pad = "  ".repeat(indent);
  const padInner = "  ".repeat(indent + 1);
  if (Array.isArray(value)) {
    if (value.length === 0) return "[]";
    const items = value.map((item) => `${padInner}${stableStringify(item, indent + 1)}`);
    return `[\n${items.join(",\n")}\n${pad}]`;
  }
  if (value !== null && typeof value === "object") {
    const keys = Object.keys(value).sort();
    if (keys.length === 0) return "{}";
    const items = keys.map(
      (key) => `${padInner}${JSON.stringify(key)}: ${stableStringify(value[key], indent + 1)}`
    );
    return `{\n${items.join(",\n")}\n${pad}}`;
  }
  return JSON.stringify(value);
}

function atomicWriteJson(filePath, value) {
  mkdirSync(path.dirname(filePath), { recursive: true });
  const temp = `${filePath}.tmp-${process.pid}`;
  writeFileSync(temp, `${stableStringify(value)}\n`);
  renameSync(temp, filePath);
}

function installReference(source, destination, replace) {
  if (existsSync(destination)) {
    if (sha256File(source) === sha256File(destination)) {
      return;
    }
    if (!replace) {
      fail(`reference exists with different bytes: ${destination}; pass --replace`);
    }
  }
  renameSync(source, destination);
}

function parseArgs(argv) {
  const args = {
    repo: ".",
    dpi: 144,
    deviceScaleFactor: 1.5,
    replace: false,
    checkOnly: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    const needsValue = () => {
      index += 1;
      if (index >= argv.length) fail(`missing value for ${flag}`);
      return argv[index];
    };
    switch (flag) {
      case "--repo":
        args.repo = needsValue();
        break;
      case "--form-code":
        args.formCode = needsValue();
        break;
      case "--revision":
        args.revision = needsValue();
        break;
      case "--pdf":
        args.pdf = needsValue();
        break;
      case "--expected-sha256":
        args.expectedSha256 = needsValue().toLowerCase();
        break;
      case "--dpi":
        args.dpi = Number(needsValue());
        break;
      case "--device-scale-factor":
        args.deviceScaleFactor = Number(needsValue());
        break;
      case "--replace":
        args.replace = true;
        break;
      case "--check-only":
        args.checkOnly = true;
        break;
      default:
        fail(`unknown argument: ${flag}`);
    }
  }
  if (!args.formCode || !args.revision || !args.pdf || !args.expectedSha256) {
    fail("--form-code, --revision, --pdf, and --expected-sha256 are required");
  }
  if (!Number.isInteger(args.dpi) || args.dpi <= 0) {
    fail("--dpi must be a positive integer");
  }
  if (!Number.isFinite(args.deviceScaleFactor) || args.deviceScaleFactor <= 0) {
    fail("--device-scale-factor must be positive");
  }
  if (!SHA256_RE.test(args.expectedSha256)) {
    fail("--expected-sha256 must be 64 lowercase hexadecimal characters");
  }
  return args;
}

function loadPopplerSourceRecord(root, slug, revision) {
  const sourceJson = path.join(
    root,
    "packages/form-renderer/references",
    `${slug}-${revision.toLowerCase()}-source.json`
  );
  if (!existsSync(sourceJson)) {
    fail(
      `Poppler reference record is missing: ${sourceJson}; run ` +
        "prepare_official_reference.py first so the chromium raster binds to pinned pages"
    );
  }
  const record = JSON.parse(readFileSync(sourceJson, "utf8"));
  const form = record.form;
  if (!form || !Array.isArray(form.pages) || form.pages.length === 0) {
    fail(`Poppler reference record has no pages: ${sourceJson}`);
  }
  return { sourceJson, form };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const root = path.resolve(args.repo);
  const pdf = path.resolve(args.pdf);
  if (!existsSync(pdf)) {
    fail(`official PDF not found: ${pdf}`);
  }
  const actualDigest = sha256File(pdf);
  if (actualDigest !== args.expectedSha256) {
    fail(
      `official PDF SHA-256 mismatch: expected ${args.expectedSha256}, got ${actualDigest}`
    );
  }

  const { code, revision, slug, formId } = identity(args.formCode, args.revision);
  const { form: popplerForm } = loadPopplerSourceRecord(root, slug, revision);
  if (popplerForm.official_source_sha256 !== actualDigest) {
    fail(
      "official PDF does not match the pinned Poppler reference record: " +
        `pinned ${popplerForm.official_source_sha256}, got ${actualDigest}`
    );
  }
  if (popplerForm.code !== code || popplerForm.revision !== revision) {
    fail("form identity does not match the pinned Poppler reference record");
  }
  if (popplerForm.dpi !== args.dpi) {
    fail(
      `--dpi ${args.dpi} does not match the pinned Poppler reference dpi ${popplerForm.dpi}`
    );
  }

  const pageCount = popplerForm.page_count;
  const widthPt = popplerForm.page_width_pt;
  const heightPt = popplerForm.page_height_pt;
  const expectedWidthPx = Math.round((widthPt * args.dpi) / 72);
  const expectedHeightPx = Math.round((heightPt * args.dpi) / 72);
  const cssWidth = expectedWidthPx / args.deviceScaleFactor;
  const cssHeight = expectedHeightPx / args.deviceScaleFactor;
  if (!Number.isInteger(cssWidth) || !Number.isInteger(cssHeight)) {
    fail(
      `page geometry ${expectedWidthPx}x${expectedHeightPx}px is not integral in CSS px at ` +
        `deviceScaleFactor ${args.deviceScaleFactor}; refusing an inexact raster`
    );
  }

  const pdftocairoVersion = requireTool("pdftocairo");
  const playwrightVersion = createRequire(import.meta.url)("playwright/package.json").version;

  const outputDir = path.join(root, "packages/form-renderer/references");
  const tempDir = mkdtempSync(path.join(existsSync(outputDir) ? outputDir : tmpdir(), "chromium-ref-"));
  const browser = await chromium.launch();
  try {
    const context = await browser.newContext({
      viewport: VIEWPORT,
      deviceScaleFactor: args.deviceScaleFactor,
    });
    const page = await context.newPage();
    const chromiumVersion = browser.version();
    const pageRecords = [];

    for (const popplerPage of popplerForm.pages) {
      const pageNumber = popplerPage.page;
      const popplerPngPath = path.join(root, popplerPage.reference_png);
      if (!existsSync(popplerPngPath)) {
        fail(`pinned Poppler reference PNG is missing: ${popplerPngPath}`);
      }
      const popplerSha = sha256File(popplerPngPath);
      if (popplerSha !== popplerPage.reference_png_sha256) {
        fail(
          `pinned Poppler reference PNG drifted on disk: ${popplerPngPath} ` +
            `(expected ${popplerPage.reference_png_sha256}, got ${popplerSha})`
        );
      }

      const svgPath = path.join(tempDir, `page-${pageNumber}.svg`);
      const conversion = spawnSync(
        "pdftocairo",
        ["-svg", "-f", String(pageNumber), "-l", String(pageNumber), pdf, svgPath],
        { encoding: "utf8" }
      );
      if (conversion.status !== 0) {
        fail(
          `pdftocairo failed for page ${pageNumber}: ${conversion.stderr || conversion.stdout}`
        );
      }
      const vectorSvgSha256 = sha256File(svgPath);

      const svgData = readFileSync(svgPath).toString("base64");
      const html =
        "<!doctype html><html><head><style>" +
        "html,body{margin:0;padding:0;background:#ffffff;}" +
        `img{display:block;width:${cssWidth}px;height:${cssHeight}px;}` +
        "</style></head><body>" +
        `<img id="page" src="data:image/svg+xml;base64,${svgData}">` +
        "</body></html>";
      await page.setContent(html, { waitUntil: "load" });
      await page.evaluate(() => document.getElementById("page").decode());
      const shot = await page.locator("#page").screenshot({ animations: "disabled" });
      const actual = PNG.sync.read(shot);
      if (actual.width !== expectedWidthPx || actual.height !== expectedHeightPx) {
        fail(
          `page ${pageNumber} pixel size mismatch: expected ` +
            `${expectedWidthPx}x${expectedHeightPx}, got ${actual.width}x${actual.height}`
        );
      }

      const expected = PNG.sync.read(readFileSync(popplerPngPath));
      if (expected.width !== expectedWidthPx || expected.height !== expectedHeightPx) {
        fail(
          `pinned Poppler reference has unexpected geometry: ${popplerPngPath} is ` +
            `${expected.width}x${expected.height}`
        );
      }
      const diff = new PNG({ width: expectedWidthPx, height: expectedHeightPx });
      const noiseFloorChangedPixels = pixelmatch(
        expected.data,
        actual.data,
        diff.data,
        expectedWidthPx,
        expectedHeightPx,
        { threshold: PIXELMATCH_THRESHOLD, includeAA: false }
      );
      const totalPixels = expectedWidthPx * expectedHeightPx;

      const chromiumPngName = `${slug}-${revision.toLowerCase()}-page-${pageNumber}-chromium.png`;
      const chromiumPngTemp = path.join(tempDir, chromiumPngName);
      writeFileSync(chromiumPngTemp, PNG.sync.write(actual));
      const chromiumPngSha256 = sha256File(chromiumPngTemp);
      const destination = path.join(outputDir, chromiumPngName);
      if (!args.checkOnly) {
        installReference(chromiumPngTemp, destination, args.replace);
      }

      pageRecords.push({
        page: pageNumber,
        chromium_png: `packages/form-renderer/references/${chromiumPngName}`,
        chromium_png_sha256: chromiumPngSha256,
        reference_width_px: expectedWidthPx,
        reference_height_px: expectedHeightPx,
        vector_svg_sha256: vectorSvgSha256,
        poppler_reference_png: popplerPage.reference_png,
        poppler_reference_png_sha256: popplerPage.reference_png_sha256,
        noise_floor_changed_pixels: noiseFloorChangedPixels,
        noise_floor_changed_percent: (noiseFloorChangedPixels / totalPixels) * 100,
        noise_floor_pixelmatch_threshold: PIXELMATCH_THRESHOLD,
      });
    }

    if (pageRecords.length !== pageCount) {
      fail(
        `rendered ${pageRecords.length} pages but the pinned record declares ${pageCount}`
      );
    }

    const record = {
      schema_version: 1,
      calibration_only: true,
      runtime_background_allowed: false,
      form: {
        code,
        revision,
        form_id: formId,
        official_source_file: popplerForm.official_source_file,
        official_source_sha256: actualDigest,
        page_count: pageCount,
        page_width_pt: widthPt,
        page_height_pt: heightPt,
        dpi: args.dpi,
        device_scale_factor: args.deviceScaleFactor,
        pages: pageRecords,
        reference_provenance: {
          kind: "official_chromium_raster",
          replacement_required: false,
          runtime_eligible: false,
        },
      },
      generator: {
        name: "prepare_chromium_reference.mjs",
        pdftocairo_version: pdftocairoVersion,
        playwright_version: playwrightVersion,
        chromium_version: chromiumVersion,
        platform: process.platform,
      },
    };
    if (!args.checkOnly) {
      atomicWriteJson(
        path.join(outputDir, `${slug}-${revision.toLowerCase()}-chromium-source.json`),
        record
      );
    }
    process.stdout.write(`${stableStringify(record)}\n`);
  } finally {
    await browser.close();
    rmSync(tempDir, { recursive: true, force: true });
  }
}

const invokedDirectly =
  process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url;
if (invokedDirectly) {
  main().catch((error) => {
    console.error(`chromium reference preparation failed: ${error.message}`);
    process.exit(1);
  });
}

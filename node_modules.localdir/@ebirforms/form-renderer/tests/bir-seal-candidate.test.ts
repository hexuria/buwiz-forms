import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { describe, expect, it } from "vitest";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const RENDERER_ROOT = path.resolve(HERE, "..");
const WORKSPACE_ROOT = path.resolve(RENDERER_ROOT, "../..");
const ARTWORK_ROOT = path.join(RENDERER_ROOT, "references/artwork");
const ORIGINAL = path.join(ARTWORK_ROOT, "bir-seal-commons-original.svg");
const EXACT_WHITE = path.join(
  ARTWORK_ROOT,
  "bir-seal-commons-binary-exact-white.svg"
);
const CANDIDATE_2551Q = path.join(
  ARTWORK_ROOT,
  "bir-seal-2551q-2018-candidate.svg"
);
const PROVENANCE = path.join(ARTWORK_ROOT, "bir-seal-commons-provenance.json");
const REFERENCE_MANIFEST = path.join(RENDERER_ROOT, "references/manifest.json");

function digest(algorithm: "sha1" | "sha256", payload: Buffer | string): string {
  return createHash(algorithm).update(payload).digest("hex");
}

function pathData(svg: string): string[] {
  return [...svg.matchAll(/<path\b[\s\S]*?\bd="([^"]*)"[\s\S]*?\/>/g)].map(
    (match) => match[1]
  );
}

function sourceFiles(root: string): string[] {
  return fs.readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(root, entry.name);
    return entry.isDirectory() ? sourceFiles(target) : [target];
  });
}

function activeBounds(file: string): [number, number, number, number] {
  const png = PNG.sync.read(fs.readFileSync(file));
  let minimumX = png.width;
  let minimumY = png.height;
  let maximumX = -1;
  let maximumY = -1;
  for (let y = 0; y < png.height; y += 1) {
    for (let x = 0; x < png.width; x += 1) {
      const offset = (y * png.width + x) * 4;
      if (png.data[offset + 3] === 0 || (
        png.data[offset] === 255
        && png.data[offset + 1] === 255
        && png.data[offset + 2] === 255
      )) continue;
      minimumX = Math.min(minimumX, x);
      minimumY = Math.min(minimumY, y);
      maximumX = Math.max(maximumX, x);
      maximumY = Math.max(maximumY, y);
    }
  }
  if (maximumX < minimumX || maximumY < minimumY) {
    throw new Error(`${file} has no non-white artwork`);
  }
  return [
    minimumX,
    minimumY,
    maximumX - minimumX + 1,
    maximumY - minimumY + 1,
  ];
}

describe("Wikimedia BIR seal exact-white candidate", () => {
  it("pins the public-domain source and preserves every vector path", () => {
    const provenance = JSON.parse(fs.readFileSync(PROVENANCE, "utf8"));
    const original = fs.readFileSync(ORIGINAL);
    const exactWhite = fs.readFileSync(EXACT_WHITE);
    const originalText = original.toString("utf8");
    const exactWhiteText = exactWhite.toString("utf8");
    const originalPaths = pathData(originalText);
    const exactWhitePaths = pathData(exactWhiteText);

    expect(digest("sha1", original)).toBe(provenance.source.source_sha1);
    expect(digest("sha256", original)).toBe(provenance.source.source_sha256);
    expect(digest("sha256", exactWhite)).toBe(provenance.derivative.sha256);
    expect(original.byteLength).toBe(provenance.source.source_bytes);
    expect(originalPaths).toHaveLength(provenance.derivative.path_count);
    expect(exactWhitePaths).toEqual(originalPaths);
    expect(digest("sha256", originalPaths.join("\0"))).toBe(
      provenance.derivative.path_data_sha256
    );
    expect(exactWhiteText).not.toMatch(/<(?:image|filter)\b/i);

    const colors = [
      ...exactWhiteText.matchAll(/#([0-9a-f]{6}|[0-9a-f]{3})\b/gi),
    ].map((match) => match[1].toLowerCase());
    expect(colors.length).toBeGreaterThan(0);
    expect(new Set(colors)).toEqual(new Set(["000000", "ffffff"]));
    expect(path.relative(WORKSPACE_ROOT, EXACT_WHITE)).toBe(
      provenance.derivative.file
    );
    expect(provenance.derivative.mode).toBe("binary");
    expect(provenance.derivative.threshold).toBe(255);
    expect(provenance.generator_support.reviewed_binary_thresholds).toEqual([
      128, 192, 255,
    ]);
  });

  it("keeps the generic candidate out of every production renderer", () => {
    const provenance = JSON.parse(fs.readFileSync(PROVENANCE, "utf8"));
    const manifestText = fs.readFileSync(REFERENCE_MANIFEST, "utf8");
    const productionSources = sourceFiles(path.join(RENDERER_ROOT, "src"))
      .filter((file) => /\.(?:css|ts|tsx)$/.test(file))
      .map((file) => fs.readFileSync(file, "utf8"))
      .join("\n");

    expect(provenance.status).toBe("calibration_candidate_only");
    expect(provenance.runtime_eligible).toBe(false);
    expect(provenance.approved_form_revisions).toEqual([]);
    expect(provenance.official_form_comparison.exact_revision_artwork_match_proven)
      .toBe(false);
    expect(manifestText).not.toContain(provenance.derivative.file);
    expect(manifestText).not.toContain(provenance.derivative.sha256);
    for (const presentation of provenance.official_form_comparison
      .form_specific_presentations) {
      expect(manifestText).not.toContain(presentation.file);
      expect(manifestText).not.toContain(presentation.sha256);
    }
    expect(productionSources).not.toContain("bir-seal-commons");
    expect(productionSources).not.toContain("bir-seal-2551q-2018-candidate");
    expect(productionSources).not.toContain(provenance.source.original_svg_url);
  });

  it("models the 2551Q canvas without copying the shared path data", () => {
    const provenance = JSON.parse(fs.readFileSync(PROVENANCE, "utf8"));
    const wrapper = fs.readFileSync(CANDIDATE_2551Q, "utf8");
    const presentation = provenance.official_form_comparison
      .form_specific_presentations.find(
        (candidate: { form: string }) => candidate.form === "2551Q:2018"
      );

    expect(presentation).toBeDefined();
    expect(digest("sha256", wrapper)).toBe(presentation.sha256);
    expect(pathData(wrapper)).toEqual([]);
    expect(wrapper).toMatch(/viewBox="0 0 95 83"/);
    expect(wrapper).toMatch(/<image[\s\S]*?x="9"[\s\S]*?y="7"/);
    expect(wrapper).toMatch(/width="75"[\s\S]*?height="73"/);
    expect(
      wrapper.match(/\.\/bir-seal-commons-binary-exact-white\.svg/g)
    ).toHaveLength(2);
    expect(presentation.path_data_duplicated).toBe(false);
    expect(presentation.canvas_pixels).toEqual([95, 83]);
    expect(presentation.active_bounds_pixels).toEqual([9, 7, 75, 73]);
    expect(presentation.calibration.result).toBe(
      "rejected_for_runtime_candidate_worsens_official_fidelity"
    );
    expect(presentation.calibration.candidate_seal_delta_pixels).toBeGreaterThan(0);
    expect(presentation.calibration.best_binary_variant).toBe(
      "binary_exact_white"
    );
    expect(presentation.calibration.best_binary_beats_grayscale).toBe(true);
    expect(presentation.calibration.best_binary_beats_current_official_pdf_xobject)
      .toBe(false);
    expect(presentation.calibration.historical_variants).toHaveLength(4);
    for (const variant of presentation.calibration.historical_variants) {
      expect(variant.seal_delta_from_current_pixels).toBeGreaterThan(0);
      expect(variant.page_delta_from_current_pixels).toBeGreaterThan(0);
    }
  });

  it("binds comparison claims to the exact reviewed PDF seal entries", () => {
    const provenance = JSON.parse(fs.readFileSync(PROVENANCE, "utf8"));
    const manifest = JSON.parse(fs.readFileSync(REFERENCE_MANIFEST, "utf8"));

    for (const comparison of provenance.official_form_comparison
      .representative_official_assets) {
      const [code, revision] = comparison.form.split(":");
      const form = manifest.forms.find(
        (candidate: { code: string; revision: string }) =>
          candidate.code === code && candidate.revision === revision
      );
      expect(form?.official_source_sha256).toBe(comparison.official_pdf_sha256);
      const seal = form?.runtime_discrete_assets.find(
        (asset: { asset: string }) => asset.asset === "government_seal"
      );
      expect(seal?.source_pdf_object_id).toEqual(comparison.source_object);
      expect(seal?.source_pixel_dimensions).toEqual(comparison.canvas_pixels);
      expect(seal?.source_bbox_top_left_points).toEqual(
        comparison.placed_bbox_points
      );
      expect(seal?.derived_png_sha256).toBe(comparison.runtime_sha256);
      expect(seal?.embedded_in).toMatch(/-seal\.png$/);
      expect(activeBounds(path.join(WORKSPACE_ROOT, seal.embedded_in))).toEqual(
        comparison.active_bounds_pixels
      );
    }

    expect(path.relative(WORKSPACE_ROOT, ORIGINAL)).toBe(
      provenance.source.source_file
    );
    expect(path.relative(WORKSPACE_ROOT, EXACT_WHITE)).toBe(
      provenance.derivative.file
    );
    const presentation = provenance.official_form_comparison
      .form_specific_presentations[0];
    expect(path.relative(WORKSPACE_ROOT, CANDIDATE_2551Q)).toBe(
      presentation.file
    );
  });
});

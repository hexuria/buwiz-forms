import crypto from "node:crypto";
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const OUTPUT_DIR = path.resolve(HERE, "../../assets/form-renderer");
const WORKSPACE_ROOT = path.resolve(HERE, "../..");
const RUNTIME_ARTWORK_SOURCE_ROOT = path.resolve(
  WORKSPACE_ROOT,
  "packages/form-renderer/src/forms"
);
const REFERENCE_MANIFEST_PATH = path.resolve(
  WORKSPACE_ROOT,
  "packages/form-renderer/references/manifest.json"
);
const REQUIRE = createRequire(import.meta.url);
const ARIMO_PACKAGE_NAME = "@fontsource-variable/arimo";
const ARIMO_PACKAGE_VERSION = "5.2.8";
const ARIMO_PACKAGE_INTEGRITY = "sha512-1Na2/dZYm/fo8m0clAmIRRKLZsF0NuKzCdtQpI7yZN6bqNTgZzroTUYIN0KA4Qk9Iov6NA/zjRdsJLmc56dv/A==";
const ARIMO_LICENSE_SHA256 = "00e06131184c7fb03bd0d1ea4b27676f1cf774da88b2e5284e43b1ec8207fd35";
const ARIMO_METADATA_SHA256 = "c8a8202239f6b3eac06d0370e0af411362ecb047208339de04cb5cf468b263aa";
const ARIMO_SOURCE_FILES = [
  {
    path: "files/arimo-latin-wght-normal.woff2",
    bytes: 20_472,
    sha256: "cceb75629f2a32e4698d087f1bb0c6c4cdc1eb9b19cd416a54cfd7323356db7e"
  },
  {
    path: "files/arimo-latin-wght-italic.woff2",
    bytes: 22_576,
    sha256: "67de270f7acd4088c49dec4cddc39bbf1bb858772cb74bbae529d4c18b7ae459"
  },
  {
    path: "files/arimo-latin-ext-wght-normal.woff2",
    bytes: 98_564,
    sha256: "86c4a19d6742fcd22c42db5891d1ab26292790607e15afe0d52674d10f9ce93d"
  },
  {
    path: "files/arimo-latin-ext-wght-italic.woff2",
    bytes: 114_412,
    sha256: "ef2f35215bd2dd8c84046cd85894c5efb869dcae21fc23791793e0c1611d00a1"
  }
] as const;
const ROBOTO_CONDENSED_PACKAGE_NAME = "@fontsource-variable/roboto-condensed";
const ROBOTO_CONDENSED_PACKAGE_VERSION = "5.2.8";
const ROBOTO_CONDENSED_PACKAGE_INTEGRITY = "sha512-aIZ2kYSoJHkTI4z8x/PRgKX6Zb9TTtSE/u+fUYeiwL+5trP9rhYYEEeNjRttaMqRgoDHcSueArdRZ43wf/i2Kw==";
const ROBOTO_CONDENSED_LICENSE_SHA256 = "82baba1bc6be17e47b138e90ab99ad134168f931282b68bc560102ab1db743c1";
const ROBOTO_CONDENSED_METADATA_SHA256 = "c132bde38ccb5f7653a9a5ff2389eb79a886b91c9eda17a77e926bb29a6adec6";
const ROBOTO_CONDENSED_SOURCE_FILES = [
  {
    path: "files/roboto-condensed-latin-wght-normal.woff2",
    bytes: 51_412,
    sha256: "8d230115e58faa2ed303bee567b91d1a792e0c958a0118998b53648b2ab7c057"
  },
  {
    path: "files/roboto-condensed-latin-wght-italic.woff2",
    bytes: 56_956,
    sha256: "c2867c1f84c8f2985f1fe41463b353d38aaf9528f55d475a6adca30ebd3c797f"
  }
] as const;

function sha256(filePath: string) {
  return crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}

function sha256Bytes(content: Buffer) {
  return crypto.createHash("sha256").update(content).digest("hex");
}

function loadAuthorizedRuntimeSealHashes() {
  const manifest = JSON.parse(fs.readFileSync(REFERENCE_MANIFEST_PATH, "utf8")) as {
    forms?: Array<{
      runtime_discrete_assets?: Array<{
        asset?: string;
        derived_png_sha256?: string;
        embedded_in?: string;
      }>;
    }>;
  };
  if (!Array.isArray(manifest.forms) || manifest.forms.length === 0) {
    throw new Error("The generated reference manifest has no forms");
  }

  const hashes = new Set<string>();
  for (const form of manifest.forms) {
    if (!Array.isArray(form.runtime_discrete_assets)) {
      throw new Error("A generated form is missing runtime_discrete_assets");
    }
    for (const asset of form.runtime_discrete_assets) {
      if (asset.asset !== "government_seal") {
        if (asset.derived_png_sha256 !== undefined) {
          throw new Error("Only exact government-seal XObjects may authorize raster bytes");
        }
        continue;
      }
      if (
        typeof asset.derived_png_sha256 !== "string"
        || !/^[0-9a-f]{64}$/u.test(asset.derived_png_sha256)
        || typeof asset.embedded_in !== "string"
      ) {
        throw new Error("A government seal has invalid generated authorization metadata");
      }
      const source = path.resolve(WORKSPACE_ROOT, asset.embedded_in);
      if (
        path.extname(source).toLowerCase() !== ".png"
        || !source.startsWith(`${RUNTIME_ARTWORK_SOURCE_ROOT}${path.sep}`)
        || !fs.existsSync(source)
        || sha256(source) !== asset.derived_png_sha256
      ) {
        throw new Error(`A government seal does not match its exact source bytes: ${asset.embedded_in}`);
      }
      hashes.add(asset.derived_png_sha256);
    }
  }
  if (hashes.size === 0) {
    throw new Error("The generated manifest authorizes no exact runtime seals");
  }
  return hashes;
}

const AUTHORIZED_RUNTIME_SEAL_SHA256 = loadAuthorizedRuntimeSealHashes();

function allFiles(root: string): string[] {
  return fs.readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const filePath = path.join(root, entry.name);
    return entry.isDirectory() ? allFiles(filePath) : [filePath];
  });
}

function installArimoNotices() {
  const packageJsonPath = REQUIRE.resolve(`${ARIMO_PACKAGE_NAME}/package.json`);
  const packageRoot = path.dirname(packageJsonPath);
  const packageMetadata = JSON.parse(fs.readFileSync(packageJsonPath, "utf8")) as {
    license?: string;
    version?: string;
  };
  if (
    packageMetadata.version !== ARIMO_PACKAGE_VERSION
    || packageMetadata.license !== "Apache-2.0"
  ) {
    throw new Error("The pinned Arimo package version or license changed");
  }

  const licensePath = path.join(packageRoot, "LICENSE");
  const metadataPath = path.join(packageRoot, "metadata.json");
  if (sha256(licensePath) !== ARIMO_LICENSE_SHA256) {
    throw new Error("The pinned Arimo license text changed");
  }
  if (sha256(metadataPath) !== ARIMO_METADATA_SHA256) {
    throw new Error("The pinned Arimo metadata changed");
  }

  for (const font of ARIMO_SOURCE_FILES) {
    const fontPath = path.join(packageRoot, font.path);
    if (fs.statSync(fontPath).size !== font.bytes || sha256(fontPath) !== font.sha256) {
      throw new Error(`The pinned Arimo font asset changed: ${font.path}`);
    }
  }

  const condensedPackageJsonPath = REQUIRE.resolve(`${ROBOTO_CONDENSED_PACKAGE_NAME}/package.json`);
  const condensedPackageRoot = path.dirname(condensedPackageJsonPath);
  const condensedPackageMetadata = JSON.parse(fs.readFileSync(condensedPackageJsonPath, "utf8")) as {
    license?: string;
    version?: string;
  };
  if (
    condensedPackageMetadata.version !== ROBOTO_CONDENSED_PACKAGE_VERSION
    || condensedPackageMetadata.license !== "OFL-1.1"
  ) {
    throw new Error("The pinned Roboto Condensed package version or license changed");
  }

  const condensedLicensePath = path.join(condensedPackageRoot, "LICENSE");
  const condensedMetadataPath = path.join(condensedPackageRoot, "metadata.json");
  if (sha256(condensedLicensePath) !== ROBOTO_CONDENSED_LICENSE_SHA256) {
    throw new Error("The pinned Roboto Condensed license text changed");
  }
  if (sha256(condensedMetadataPath) !== ROBOTO_CONDENSED_METADATA_SHA256) {
    throw new Error("The pinned Roboto Condensed metadata changed");
  }
  for (const font of ROBOTO_CONDENSED_SOURCE_FILES) {
    const fontPath = path.join(condensedPackageRoot, font.path);
    if (fs.statSync(fontPath).size !== font.bytes || sha256(fontPath) !== font.sha256) {
      throw new Error(`The pinned Roboto Condensed font asset changed: ${font.path}`);
    }
  }

  const builtFonts = allFiles(OUTPUT_DIR).filter((filePath) =>
    /\.(?:woff2?|ttf|otf)$/iu.test(filePath)
  );
  const unsupportedFont = builtFonts.find((filePath) => !filePath.endsWith(".woff2"));
  if (unsupportedFont) {
    throw new Error(`Production renderer contains a non-WOFF2 font: ${unsupportedFont}`);
  }
  const expectedFontCount = ARIMO_SOURCE_FILES.length + ROBOTO_CONDENSED_SOURCE_FILES.length;
  if (builtFonts.length !== expectedFontCount) {
    throw new Error(
      `Production renderer must contain exactly ${expectedFontCount} reviewed WOFF2 assets; found ${builtFonts.length}`
    );
  }

  const shippedFonts = ARIMO_SOURCE_FILES.map((font) => {
    const shippedPath = builtFonts.find((filePath) => sha256(filePath) === font.sha256);
    if (!shippedPath) {
      throw new Error(`Production renderer is missing pinned Arimo asset: ${font.path}`);
    }
    return {
      ...font,
      shipped_path: path.relative(OUTPUT_DIR, shippedPath).split(path.sep).join("/")
    };
  });
  const shippedCondensedFonts = ROBOTO_CONDENSED_SOURCE_FILES.map((font) => {
    const shippedPath = builtFonts.find((filePath) => sha256(filePath) === font.sha256);
    if (!shippedPath) {
      throw new Error(`Production renderer is missing pinned Roboto Condensed asset: ${font.path}`);
    }
    return {
      ...font,
      shipped_path: path.relative(OUTPUT_DIR, shippedPath).split(path.sep).join("/")
    };
  });

  const noticeDirectory = path.join(OUTPUT_DIR, "third-party/arimo");
  fs.mkdirSync(noticeDirectory, { recursive: true });
  fs.copyFileSync(licensePath, path.join(noticeDirectory, "LICENSE.txt"));
  fs.writeFileSync(
    path.join(noticeDirectory, "PROVENANCE.json"),
    `${JSON.stringify({
      schema_version: 1,
      family: "Arimo",
      font_version: "v35",
      package: `${ARIMO_PACKAGE_NAME}@${ARIMO_PACKAGE_VERSION}`,
      package_integrity: ARIMO_PACKAGE_INTEGRITY,
      package_repository: "https://github.com/fontsource/font-files/tree/main/fonts/variable/arimo",
      upstream_repository: "https://github.com/googlefonts/arimo",
      source_catalog: "https://github.com/google/fonts/tree/main/ofl/arimo",
      license: "Apache-2.0",
      license_sha256: ARIMO_LICENSE_SHA256,
      attribution: "Copyright 2020 The Arimo Project Authors (https://github.com/googlefonts/arimo)",
      subsets: ["latin", "latin-ext"],
      styles: ["normal", "italic"],
      weight_range: [400, 700],
      files: shippedFonts
    }, null, 2)}\n`
  );

  const condensedNoticeDirectory = path.join(OUTPUT_DIR, "third-party/roboto-condensed");
  fs.mkdirSync(condensedNoticeDirectory, { recursive: true });
  fs.copyFileSync(condensedLicensePath, path.join(condensedNoticeDirectory, "LICENSE.txt"));
  fs.writeFileSync(
    path.join(condensedNoticeDirectory, "PROVENANCE.json"),
    `${JSON.stringify({
      schema_version: 1,
      family: "Roboto Condensed",
      font_version: "v31",
      package: `${ROBOTO_CONDENSED_PACKAGE_NAME}@${ROBOTO_CONDENSED_PACKAGE_VERSION}`,
      package_integrity: ROBOTO_CONDENSED_PACKAGE_INTEGRITY,
      package_repository: "https://github.com/fontsource/font-files/tree/main/fonts/variable/roboto-condensed",
      upstream_repository: "https://github.com/googlefonts/roboto-3-classic",
      source_catalog: "https://github.com/google/fonts/tree/main/ofl/robotocondensed",
      license: "OFL-1.1",
      license_sha256: ROBOTO_CONDENSED_LICENSE_SHA256,
      attribution: "Copyright 2011 Google Inc. All Rights Reserved.",
      subsets: ["latin"],
      styles: ["normal", "italic"],
      weight_range: [100, 900],
      files: shippedCondensedFonts
    }, null, 2)}\n`
  );
}

export default defineConfig({
  plugins: [
    react(),
    {
      name: "file-protocol-compatible-entry-and-font-notices",
      closeBundle() {
        const indexPath = path.join(OUTPUT_DIR, "index.html");
        const html = fs.readFileSync(indexPath, "utf8");
        const compatible = html.replace(
          '<script type="module" crossorigin',
          "<script defer"
        );
        if (compatible === html) {
          throw new Error("Vite output did not contain the expected module entry");
        }
        fs.writeFileSync(indexPath, compatible);
        installArimoNotices();
      }
    }
  ],
  base: "./",
  build: {
    // The native host executes a classic script from a custom/file-compatible
    // local origin. Inline only the exact native seal bytes authorized by the
    // generated manifest so Vite never emits new URL(..., import.meta.url).
    // Every other image remains unauthorized and the offline verifier still
    // rejects it by content hash, independent of its filename.
    assetsInlineLimit(filePath, content) {
      const source = path.resolve(filePath);
      return (
        source.startsWith(`${RUNTIME_ARTWORK_SOURCE_ROOT}${path.sep}`)
        && AUTHORIZED_RUNTIME_SEAL_SHA256.has(sha256Bytes(content))
      );
    },
    modulePreload: {
      polyfill: false
    },
    outDir: OUTPUT_DIR,
    emptyOutDir: true
  }
});

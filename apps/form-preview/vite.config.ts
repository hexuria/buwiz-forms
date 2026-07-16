import crypto from "node:crypto";
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const OUTPUT_DIR = path.resolve(HERE, "../../assets/form-renderer");
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

function sha256(filePath: string) {
  return crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}

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

  const builtFonts = allFiles(OUTPUT_DIR).filter((filePath) =>
    /\.(?:woff2?|ttf|otf)$/iu.test(filePath)
  );
  const unsupportedFont = builtFonts.find((filePath) => !filePath.endsWith(".woff2"));
  if (unsupportedFont) {
    throw new Error(`Production renderer contains a non-WOFF2 font: ${unsupportedFont}`);
  }
  if (builtFonts.length !== ARIMO_SOURCE_FILES.length) {
    throw new Error(
      `Production renderer must contain exactly ${ARIMO_SOURCE_FILES.length} Arimo WOFF2 assets; found ${builtFonts.length}`
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
    modulePreload: {
      polyfill: false
    },
    outDir: OUTPUT_DIR,
    emptyOutDir: true
  }
});

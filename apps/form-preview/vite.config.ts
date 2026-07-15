import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const OUTPUT_DIR = path.resolve(HERE, "../../assets/form-renderer");

export default defineConfig({
  plugins: [
    react(),
    {
      name: "file-protocol-compatible-entry",
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

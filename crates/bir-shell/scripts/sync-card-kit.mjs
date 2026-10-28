#!/usr/bin/env node
// Copy mini-apps/card-kit.tsx.in into every card app (mini-apps/card-*.tsx),
// replacing its `// card-kit:begin` … `// card-kit:end` block (or a
// `// @@CARD-KIT@@` line). An app may import only nsh, nsh/base and rox, so
// the kit travels inside each app; v1-tests/apps.test.mjs fails when one
// drifts. Run it after editing the kit, then re-sign the manifest.
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = join(dirname(fileURLToPath(import.meta.url)), "..", "mini-apps");
const kit = readFileSync(join(dir, "card-kit.tsx.in"), "utf8").trimEnd();
for (const file of readdirSync(dir).filter((f) => /^card-.*\.tsx$/.test(f))) {
  const path = join(dir, file);
  const source = readFileSync(path, "utf8");
  const next = source.includes("// @@CARD-KIT@@")
    ? source.replace("// @@CARD-KIT@@", kit)
    : source.replace(/\/\/ card-kit:begin[\s\S]*?\/\/ card-kit:end/, kit);
  if (next !== source) writeFileSync(path, next);
  console.log(`${file}: ${next === source ? "up to date" : "kit copied"}`);
}

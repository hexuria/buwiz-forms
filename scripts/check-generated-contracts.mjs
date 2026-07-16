import { spawnSync } from "node:child_process";

const generatedPaths = [
  "packages/form-specs/generated/form-capabilities.json",
  "packages/form-contracts/schema",
  "packages/form-contracts/fixtures",
  "packages/form-contracts/src/generated/2551q-atc-reference.json",
  "packages/form-contracts/src/generated.ts"
];

function git(args, options = {}) {
  const result = spawnSync("git", args, {
    cwd: process.cwd(),
    encoding: "utf8",
    ...options
  });
  if (result.error) {
    console.error(`Unable to run git: ${result.error.message}`);
    process.exit(1);
  }
  return result;
}

const diff = git(["diff", "--exit-code", "--", ...generatedPaths], {
  stdio: "inherit",
  encoding: undefined
});
if (diff.status !== 0) {
  console.error("Generated renderer contracts differ from the committed files.");
  process.exit(diff.status ?? 1);
}

const untracked = git(["ls-files", "--others", "--", ...generatedPaths]);
if (untracked.status !== 0) {
  process.stderr.write(untracked.stderr);
  process.exit(untracked.status ?? 1);
}

const untrackedFiles = untracked.stdout
  .split(/\r?\n/)
  .map((line) => line.trim())
  .filter(Boolean);
if (untrackedFiles.length > 0) {
  console.error("Generated renderer contracts are untracked:");
  for (const file of untrackedFiles) console.error(`- ${file}`);
  process.exit(1);
}

console.log("Generated renderer contracts match tracked files.");

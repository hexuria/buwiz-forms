import { spawnSync } from "node:child_process";

const requestedArguments = process.argv.slice(2);
if (requestedArguments.length === 0) {
  console.error("Usage: node scripts/run_python.mjs <script> [arguments...]");
  process.exit(2);
}

const candidates = process.platform === "win32"
  ? [
      ["python", []],
      ["py", ["-3"]],
      ["python3", []]
    ]
  : [
      ["python3", []],
      ["python", []]
    ];

for (const [command, prefix] of candidates) {
  const result = spawnSync(command, [...prefix, ...requestedArguments], {
    stdio: "inherit"
  });
  if (result.error?.code === "ENOENT") continue;
  if (result.error) {
    console.error(`Unable to run ${command}: ${result.error.message}`);
    process.exit(1);
  }
  process.exit(result.status ?? 1);
}

console.error("Python 3 is required but no supported interpreter was found.");
process.exit(1);

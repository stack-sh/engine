import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";

const packResult = JSON.parse(
  execFileSync(
    "npm",
    ["pack", "--dry-run", "--json", "--workspace", "@stack-sh/engine"],
    { encoding: "utf8" },
  ),
);
const packages = Array.isArray(packResult)
  ? packResult
  : Object.values(packResult);
assert.equal(packages.length, 1);
assert.equal(packages[0].name, "@stack-sh/engine");
assert.deepEqual(
  packages[0].files.map(({ path }) => path).sort(),
  [
    "LICENSE",
    "README.md",
    "THIRD_PARTY_LICENSES.md",
    "dist/stack_engine.d.ts",
    "dist/stack_engine.js",
    "dist/stack_engine_bg.wasm",
    "dist/stack_engine_bg.wasm.d.ts",
    "index.d.ts",
    "index.js",
    "licenses/MIT.txt",
    "licenses/Unicode-3.0.txt",
    "package.json",
  ],
);
console.log(`validated ${packages[0].entryCount} npm package entries`);

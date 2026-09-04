import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const packageRoot = new URL("../packages/engine/", import.meta.url);
const repositoryRoot = new URL("../", import.meta.url);
const packageDocument = JSON.parse(readFileSync(new URL("package.json", packageRoot), "utf8"));
const declaration = readFileSync(new URL("dist/stack_engine.d.ts", packageRoot), "utf8");
const glue = readFileSync(new URL("dist/stack_engine.js", packageRoot), "utf8");
const binary = readFileSync(new URL("dist/stack_engine_bg.wasm", packageRoot));

assert.equal(packageDocument.name, "@stack-sh/engine");
assert.equal(packageDocument.license, "Apache-2.0");
assert.equal(packageDocument.exports["."].types, "./index.d.ts");
assert.equal(
  readFileSync(new URL("LICENSE", packageRoot), "utf8"),
  readFileSync(new URL("LICENSE", repositoryRoot), "utf8"),
);
assert.match(declaration, /export type StackSource = string \| Uint8Array;/);
assert.match(declaration, /export function format\(source: StackSource\): FormatResult;/);
assert.match(declaration, /export function check\(source: StackSource\): CheckResult;/);
assert.match(declaration, /export function render\(source: StackSource\): RenderResult;/);
assert.match(declaration, /export function checkWithProviderPacks/);
assert.match(declaration, /export function renderWithProviderPacks/);

const module = new WebAssembly.Module(binary);
const imports = WebAssembly.Module.imports(module);
const forbiddenCapability = /(fetch|xmlhttprequest|websocket|document|window|navigator|location|storage|date|performance|crypto|random|timer|timeout|interval|process|require|filesystem|wasi|path|environment|clock)/i;
for (const imported of imports) {
  assert.equal(imported.module, "./stack_engine_bg.js");
  assert.match(imported.name, /^__(?:wbindgen|wbg)_/);
  assert.doesNotMatch(imported.name, forbiddenCapability);
}

const importGlueStart = glue.indexOf("function __wbg_get_imports()");
const importGlueEnd = glue.indexOf("function addToExternrefTable0", importGlueStart);
assert.notEqual(importGlueStart, -1);
assert.notEqual(importGlueEnd, -1);
const importGlue = glue.slice(importGlueStart, importGlueEnd);
assert.doesNotMatch(
  importGlue,
  /\b(?:eval|Function|fetch|XMLHttpRequest|WebSocket|document|window|navigator|location|localStorage|sessionStorage|Date|performance|crypto|process|require|setTimeout|setInterval)\b/,
);
for (const requiredPrimitive of ["Array", "Error", "JSON", "Object", "Reflect", "TypeError", "Uint8Array"]) {
  assert.match(importGlue, new RegExp(`\\b${requiredPrimitive}\\b`));
}

const exports = new Set(WebAssembly.Module.exports(module).map(({ name }) => name));
for (const operation of ["format", "check", "render", "checkWithProviderPacks", "renderWithProviderPacks"]) {
  assert.ok(exports.has(operation), `missing ${operation} WebAssembly export`);
}

console.log(`validated ${imports.length} capability-limited WebAssembly imports`);

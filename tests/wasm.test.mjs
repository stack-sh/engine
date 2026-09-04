import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import init, {
  check,
  checkWithProviderPacks,
  format,
  render,
  renderWithProviderPacks,
} from "../packages/engine/index.js";

const repositoryRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const fixturePath = join(repositoryRoot, "tests/fixtures/operation-cases.json");
const wasmPath = join(repositoryRoot, "packages/engine/dist/stack_engine_bg.wasm");
const cases = JSON.parse(readFileSync(fixturePath, "utf8"));
const providerPacks = JSON.parse(
  readFileSync(join(repositoryRoot, "tests/fixtures/provider-pack-input.json"), "utf8"),
);

await init({ module_or_path: readFileSync(wasmPath) });

function browserInput(input) {
  if (input.kind === "string") {
    return input.value;
  }
  return Uint8Array.from(input.value);
}

function wasmOutputs() {
  return cases.map((fixture) => {
    const source = browserInput(fixture.input);
    return {
      name: fixture.name,
      format: format(source),
      check: check(source),
      render: render(source),
    };
  });
}

test("browser exports match native engine results for shared fixtures", () => {
  const native = JSON.parse(
    execFileSync(
      "cargo",
      [
        "run",
        "--quiet",
        "--locked",
        "-p",
        "stack-engine-wasm",
        "--example",
        "native-parity",
        "--",
        fixturePath,
      ],
      { cwd: repositoryRoot, encoding: "utf8" },
    ),
  );
  assert.deepEqual(wasmOutputs(), native);
});

test("invalid UTF-8 is a normal diagnostic result for every operation", () => {
  const invalid = wasmOutputs().find(({ name }) => name === "invalid-utf8-bytes");
  assert.ok(invalid);
  assert.equal(invalid.format.formattedSource, null);
  assert.equal(invalid.render.svg, null);
  for (const operation of [invalid.format, invalid.check, invalid.render]) {
    assert.equal(operation.diagnostics[0].code, "STK1001");
    assert.equal(operation.diagnostics[0].severity, "error");
    assert.equal(operation.metadata.languageVersion, null);
  }
});

test("browser diagnostics preserve actionable compiler guidance", () => {
  const actionable = wasmOutputs().find(
    ({ name }) => name === "actionable-error-string",
  );
  assert.ok(actionable);
  assert.equal(actionable.render.svg, null);
  assert.equal(actionable.check.metadata.engineVersion, "0.5.0");
  assert.deepEqual(actionable.check.diagnostics[0], {
    code: "STK2002",
    severity: "error",
    message: "Unknown layout direction 'hoo'.",
    range: {
      start: { byteOffset: 68, line: 4, column: 22 },
      end: { byteOffset: 71, line: 4, column: 25 },
    },
    expected: ["right", "down"],
    help: "Use 'right' for horizontal flow or 'down' for vertical flow.",
    related: [],
  });
});

test("browser rendering resolves the bundled explicit core icon", () => {
  const explicitIcon = wasmOutputs().find(
    ({ name }) => name === "explicit-core-icon-string",
  );
  assert.ok(explicitIcon);
  assert.deepEqual(explicitIcon.check.diagnostics, []);
  assert.deepEqual(explicitIcon.render.diagnostics, []);
  assert.equal(explicitIcon.render.metadata.engineVersion, "0.5.0");
  assert.equal(explicitIcon.render.metadata.themeCatalogVersion, "0.4.0");
  assert.equal(
    explicitIcon.render.metadata.themeCatalogRevision,
    "sha256:9cb3de8b504acbf22c93cea5fbea66be50f38734dc1dee18b9cab7084082cc1f",
  );
  assert.match(explicitIcon.render.svg, /data-icon-id="gateway"/);
  assert.doesNotMatch(explicitIcon.render.svg, /data-icon-id="kind-external"/);
});

test("browser rendering resolves local provider packs with native provenance", () => {
  const source =
    'stack 1.0 diagram "Provider" { node item "Example Storage" { kind queue icon "example:storage" } }';
  const checked = checkWithProviderPacks(source, providerPacks);
  const rendered = renderWithProviderPacks(source, providerPacks);
  assert.deepEqual(checked.diagnostics, []);
  assert.deepEqual(rendered.diagnostics, []);
  assert.match(rendered.svg, /data-node-kind="queue"/);
  assert.match(rendered.svg, /data-icon-id="example:storage"/);
  assert.match(rendered.svg, /fill="#4285f4"/);
  assert.equal(rendered.providerNotices.length, 1);
  assert.equal(rendered.providerNotices[0].providerId, "example");
  assert.deepEqual(rendered.providerNotices[0].icons, [
    { id: "example:storage", productName: "Example Storage" },
  ]);
  assert.match(rendered.providerNotices[0].packRevision, /^sha256:[0-9a-f]{64}$/);
});

test("the JavaScript boundary rejects unsupported source values consistently", () => {
  for (const operation of [format, check, render]) {
    assert.throws(
      () => operation({ source: "not a supported boundary value" }),
      { name: "TypeError", message: "Stack source must be a string or Uint8Array" },
    );
  }
  assert.throws(
    () => checkWithProviderPacks("stack 1.0", () => undefined),
    { name: "TypeError", message: "Provider packs must be JSON-compatible local data" },
  );
});

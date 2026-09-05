import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";
import test from "node:test";

import init, {
  check,
  checkWithProviderPacks,
  completion,
  completionWithProviderPacks,
  format,
  hover,
  render,
  renderWithProviderPacks,
} from "../packages/engine/index.js";

const repositoryRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const fixturePath = join(repositoryRoot, "tests/fixtures/operation-cases.json");
const languageFixturePath = join(
  repositoryRoot,
  "tests/fixtures/language-intelligence-cases.json",
);
const providerFixturePath = join(repositoryRoot, "crates/stack-engine/tests/fixtures/provider-pack-input.json");
const wasmPath = join(repositoryRoot, "packages/engine/dist/stack_engine_bg.wasm");
const cases = JSON.parse(readFileSync(fixturePath, "utf8"));
const languageCases = JSON.parse(readFileSync(languageFixturePath, "utf8"));
const providerPacks = JSON.parse(readFileSync(providerFixturePath, "utf8"));

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

function sourceAndPosition(sourceWithCursor) {
  const marker = sourceWithCursor.indexOf("<|>");
  assert.notEqual(marker, -1);
  assert.equal(sourceWithCursor.indexOf("<|>", marker + 3), -1);
  const source = sourceWithCursor.slice(0, marker) + sourceWithCursor.slice(marker + 3);
  const prefix = source.slice(0, marker);
  const lines = prefix.split(/\r\n|\n/);
  return {
    source,
    position: {
      byteOffset: new TextEncoder().encode(prefix).length,
      line: lines.length,
      column: Array.from(lines.at(-1) ?? "").length + 1,
    },
  };
}

function wasmLanguageOutputs() {
  return languageCases.map((fixture) => {
    const { source, position } = sourceAndPosition(fixture.sourceWithCursor);
    return {
      name: fixture.name,
      completion: fixture.providerPacks
        ? completionWithProviderPacks(
            source,
            fixture.documentVersion,
            position,
            providerPacks,
          )
        : completion(source, fixture.documentVersion, position),
      hover: hover(source, fixture.documentVersion, position),
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

test("browser language intelligence matches native results for shared fixtures", () => {
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
        "language-intelligence-parity",
        "--",
        languageFixturePath,
        providerFixturePath,
      ],
      { cwd: repositoryRoot, encoding: "utf8" },
    ),
  );
  const browser = wasmLanguageOutputs();
  assert.deepEqual(browser, native);
  assert.deepEqual(
    browser.find(({ name }) => name === "diagram-keyword")?.completion.items.map(
      ({ label }) => label,
    ),
    ["node"],
  );
  assert.deepEqual(
    browser.find(({ name }) => name === "core-icon")?.completion.items.map(
      ({ filterText }) => filterText,
    ),
    ["gateway"],
  );
  assert.deepEqual(
    browser.find(({ name }) => name === "provider-icon")?.completion.items.map(
      ({ filterText }) => filterText,
    ),
    ["example:storage"],
  );
  assert.equal(
    browser.find(({ name }) => name === "multilingual-hover")?.hover.hover?.label,
    "顧客",
  );
});

test("provider-aware browser completion stays within the editor latency budget", (context) => {
  const fixture = languageCases.find(({ name }) => name === "provider-icon");
  assert.ok(fixture);
  const { source, position } = sourceAndPosition(fixture.sourceWithCursor);
  for (let index = 0; index < 5; index += 1) {
    completionWithProviderPacks(source, index, position, providerPacks);
  }
  const durations = [];
  const suiteStarted = performance.now();
  for (let index = 0; index < 100; index += 1) {
    const started = performance.now();
    const result = completionWithProviderPacks(source, index, position, providerPacks);
    durations.push(performance.now() - started);
    assert.equal(result.documentVersion, index);
    assert.equal(result.items[0]?.filterText, "example:storage");
  }
  const suiteMilliseconds = performance.now() - suiteStarted;
  durations.sort((left, right) => left - right);
  const p95Milliseconds = durations[Math.ceil(durations.length * 0.95) - 1];
  assert.ok(p95Milliseconds <= 20, `p95 ${p95Milliseconds.toFixed(3)} ms exceeded 20 ms`);
  assert.ok(suiteMilliseconds <= 500, `suite ${suiteMilliseconds.toFixed(3)} ms exceeded 500 ms`);
  context.diagnostic(
    `100 provider completions: ${suiteMilliseconds.toFixed(3)} ms suite, ${p95Milliseconds.toFixed(3)} ms p95`,
  );
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
  assert.equal(actionable.check.metadata.engineVersion, "0.7.0");
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
  assert.equal(explicitIcon.render.metadata.engineVersion, "0.7.0");
  assert.equal(explicitIcon.render.metadata.themeCatalogVersion, "0.5.0");
  assert.equal(
    explicitIcon.render.metadata.themeCatalogRevision,
    "sha256:3bfd66e1a96628b29b95b7273b54373bcce952f7285aefa506b4255a629eaf53",
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
  assert.deepEqual(rendered.providerNotices[0].sources, [
    {
      id: "primary",
      pageUrl: "https://example.com/icons",
      release: "fixture-1",
      archiveSha256:
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
      termsUrl: "https://example.com/terms",
    },
  ]);
  assert.deepEqual(rendered.providerNotices[0].icons, [
    { id: "example:storage", productName: "Example Storage", sourceId: "primary" },
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
  assert.throws(
    () => completion(new Uint8Array(), 1, { byteOffset: 0, line: 1, column: 1 }),
    { name: "TypeError", message: "Language intelligence source must be a string" },
  );
  assert.throws(() => completion("stack 1.0", -1, { byteOffset: 0, line: 1, column: 1 }), {
    name: "TypeError",
    message: "Document version must be a safe integer",
  });
  assert.throws(() => hover("stack 1.0", 1, null), {
    name: "TypeError",
    message: "Source position must be an object",
  });
  assert.throws(
    () => hover("stack 1.0", 1, { byteOffset: 0, line: 0, column: 1 }),
    { name: "TypeError", message: "Source position line must be a safe integer" },
  );
});

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { renderGallery } from "./build-layout-gallery.mjs";
import { validateCatalogDocument } from "./validate-layout-corpus.mjs";

const catalog = JSON.parse(await readFile(new URL("../layout-corpus/catalog.json", import.meta.url)));
const schema = JSON.parse(await readFile(new URL("../layout-corpus/schema.json", import.meta.url)));
const packageDocument = JSON.parse(await readFile(new URL("../package.json", import.meta.url)));
const engineVersion = packageDocument.version;

test("the checked-in layout catalog matches its versioned schema", () => {
  assert.equal(validateCatalogDocument(structuredClone(catalog), schema, engineVersion).cases.length, 7);
});

test("schema version 1 rejects unknown fields and invalid paths", () => {
  const unknown = structuredClone(catalog);
  unknown.cases[0].undocumented = true;
  assert.throws(() => validateCatalogDocument(unknown, schema, engineVersion), /additional properties/);

  const unsafe = structuredClone(catalog);
  unsafe.cases[0].source = "../outside.stack";
  assert.throws(() => validateCatalogDocument(unsafe, schema, engineVersion), /must match pattern/);
});

test("duplicate cases and missing required coverage are rejected", () => {
  const duplicate = structuredClone(catalog);
  duplicate.cases[1].id = duplicate.cases[0].id;
  assert.throws(() => validateCatalogDocument(duplicate, schema, engineVersion), /IDs must be unique/);

  const missing = structuredClone(catalog);
  for (const layoutCase of missing.cases) {
    layoutCase.features = layoutCase.features.filter((feature) => feature !== "long-labels");
  }
  assert.throws(() => validateCatalogDocument(missing, schema, engineVersion), /cover long-labels/);
});

test("provider fixtures and declared provider coverage cannot drift", () => {
  const invalid = structuredClone(catalog);
  invalid.cases.at(-1).providerFixture = null;
  assert.throws(() => validateCatalogDocument(invalid, schema, engineVersion), /must agree/);
});

test("the static gallery escapes source and exposes accessible comparisons", () => {
  const fixtureCatalog = {
    engineVersion: "0.6.0",
    schemaVersion: "1.0",
    cases: [{ id: "fixture" }],
  };
  const layoutCase = {
    id: "fixture",
    title: "A < B",
    summary: "Safe & local",
    density: "small",
    features: ["edge-labels"],
    expected: { nodes: 2, groups: 0, edges: 1 },
    alt: 'Diagram "fixture"',
    sourceText: "node <unsafe>",
  };
  const performance = { suiteMilliseconds: 1, maxP95Milliseconds: 50 };
  const html = renderGallery(
    fixtureCatalog,
    [{ layoutCase, dimensions: "1 × 1", matches: false, performanceCase: null }],
    performance,
  );
  assert.match(html, /1 changed candidates/);
  assert.match(html, /Candidate differs from approved geometry/);
  assert.match(html, /Approved reference/);
  assert.match(html, /Current engine/);
  assert.match(html, /Approved reference: Diagram &quot;fixture&quot;/);
  assert.match(html, /node &lt;unsafe&gt;/);
  assert.doesNotMatch(html, /<script\b|(?:src|href)="https?:\/\//i);
});

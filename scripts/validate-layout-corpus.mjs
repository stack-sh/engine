import assert from "node:assert/strict";
import { lstat, readFile, readdir } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

import Ajv2020 from "ajv/dist/2020.js";

const repositoryRoot = path.resolve(fileURLToPath(new URL("../", import.meta.url)));
const corpusRoot = path.join(repositoryRoot, "layout-corpus");
const requiredDensities = ["small", "medium", "dense"];
const requiredFeatures = [
  "groups",
  "nested-groups",
  "rank-constraints",
  "order-constraints",
  "cross-edges",
  "edge-labels",
  "long-labels",
  "provider-icons",
];

export function validateCatalogDocument(catalog, schema, engineVersion) {
  const ajv = new Ajv2020({ allErrors: true, strict: true });
  assert.equal(ajv.validateSchema(schema), true, formatErrors(ajv.errors));
  const validate = ajv.compile(schema);
  assert.equal(validate(catalog), true, formatErrors(validate.errors));
  assert.equal(catalog.engineVersion, engineVersion, "layout corpus engineVersion drift");

  const ids = catalog.cases.map(({ id }) => id);
  const sources = catalog.cases.map(({ source }) => source);
  const snapshots = catalog.cases.map(({ snapshot }) => snapshot);
  assert.equal(new Set(ids).size, ids.length, "layout corpus case IDs must be unique");
  assert.equal(new Set(sources).size, sources.length, "layout corpus source paths must be unique");
  assert.equal(
    new Set(snapshots).size,
    snapshots.length,
    "layout corpus snapshot paths must be unique",
  );

  const densities = new Set(catalog.cases.map(({ density }) => density));
  const features = new Set(catalog.cases.flatMap(({ features: values }) => values));
  for (const density of requiredDensities) {
    assert.ok(densities.has(density), `layout corpus must cover ${density} density`);
  }
  for (const feature of requiredFeatures) {
    assert.ok(features.has(feature), `layout corpus must cover ${feature}`);
  }

  for (const layoutCase of catalog.cases) {
    assert.equal(
      layoutCase.source,
      `sources/${layoutCase.id}.stack`,
      `${layoutCase.id} source path must follow its ID`,
    );
    assert.equal(
      layoutCase.snapshot,
      `snapshots/${layoutCase.id}.svg`,
      `${layoutCase.id} snapshot path must follow its ID`,
    );
    const usesProvider = layoutCase.features.includes("provider-icons");
    assert.equal(
      layoutCase.providerFixture !== null,
      usesProvider,
      `${layoutCase.id} provider fixture and feature must agree`,
    );
  }
  return catalog;
}

export async function validateRepositoryLayoutCorpus() {
  const [catalog, schema, packageDocument] = await Promise.all([
    readJson(path.join(corpusRoot, "catalog.json")),
    readJson(path.join(corpusRoot, "schema.json")),
    readJson(path.join(repositoryRoot, "package.json")),
  ]);
  validateCatalogDocument(catalog, schema, packageDocument.version);

  const expectedSources = catalog.cases.map(({ source }) => path.basename(source)).sort();
  const expectedSnapshots = catalog.cases.map(({ snapshot }) => path.basename(snapshot)).sort();
  const actualSources = await inventory(path.join(corpusRoot, "sources"), ".stack");
  const actualSnapshots = await inventory(path.join(corpusRoot, "snapshots"), ".svg");
  assert.deepEqual(actualSources, expectedSources, "layout source inventory drift");
  assert.deepEqual(actualSnapshots, expectedSnapshots, "layout snapshot inventory drift");

  for (const layoutCase of catalog.cases) {
    const [source, snapshot] = await Promise.all([
      readBoundedRegularFile(path.join(corpusRoot, layoutCase.source), 64 * 1024),
      readBoundedRegularFile(path.join(corpusRoot, layoutCase.snapshot), 1024 * 1024),
    ]);
    assert.match(source, /^stack 1\.0\n/, `${layoutCase.id} has no language header`);
    assert.doesNotMatch(source, /\r/, `${layoutCase.id} must use LF newlines`);
    assert.match(snapshot, /^<\?xml version="1\.0" encoding="UTF-8"\?>\n<svg\b/);
    assert.match(snapshot, new RegExp(`data-engine-version="${escapeRegex(catalog.engineVersion)}"`));
    assert.doesNotMatch(snapshot, /<script\b|<foreignObject\b|(?:href|src)="https?:/i);
    if (layoutCase.providerFixture !== null) {
      await readBoundedRegularFile(path.resolve(corpusRoot, layoutCase.providerFixture), 1024 * 1024);
    }
  }

  return { catalog, corpusRoot, repositoryRoot };
}

async function inventory(directory, extension) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    assert.ok(entry.isFile(), `${path.join(directory, entry.name)} must be a regular file`);
    assert.equal(
      path.extname(entry.name),
      extension,
      `${path.join(directory, entry.name)} has an unexpected extension`,
    );
  }
  return entries.map(({ name }) => name).sort();
}

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

async function readBoundedRegularFile(file, maximumBytes) {
  const metadata = await lstat(file);
  assert.ok(metadata.isFile() && !metadata.isSymbolicLink(), `${file} must be a regular file`);
  assert.ok(metadata.size > 0 && metadata.size <= maximumBytes, `${file} has an invalid size`);
  return readFile(file, "utf8");
}

function formatErrors(errors) {
  return errors?.map((error) => `${error.instancePath || "/"} ${error.message}`).join("; ") ?? "";
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;
if (invokedPath === import.meta.url) {
  const { catalog } = await validateRepositoryLayoutCorpus();
  console.log(
    `Validated ${catalog.cases.length} versioned layout cases across ${requiredDensities.length} densities and ${requiredFeatures.length} required failure-mode features.`,
  );
}

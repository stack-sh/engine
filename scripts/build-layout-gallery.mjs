import assert from "node:assert/strict";
import { copyFile, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

import { validateRepositoryLayoutCorpus } from "./validate-layout-corpus.mjs";

const repositoryRoot = path.resolve(fileURLToPath(new URL("../", import.meta.url)));

export function renderGallery(catalog, comparisons, performance) {
  const changed = comparisons.filter((comparison) => !comparison.matches).length;
  const cards = comparisons
    .map(({ layoutCase, dimensions, matches, performanceCase }) => {
      const features = layoutCase.features
        .map((feature) => `<li>${escapeHtml(feature)}</li>`)
        .join("");
      const performanceText = performanceCase
        ? `${performanceCase.p95Milliseconds.toFixed(3)} ms p95`
        : "Not measured";
      return `
        <article class="case ${matches ? "matches" : "changed"}">
          <header>
            <div>
              <p class="eyebrow">${escapeHtml(layoutCase.density)} · ${dimensions}</p>
              <h2>${escapeHtml(layoutCase.title)}</h2>
              <p>${escapeHtml(layoutCase.summary)}</p>
            </div>
            <p class="result">${matches ? "Matches approved geometry" : "Candidate differs from approved geometry"}</p>
          </header>
          <ul class="features" aria-label="Covered layout features">${features}</ul>
          <div class="comparison">
            <figure>
              <figcaption>Approved reference</figcaption>
              <img alt="Approved reference: ${escapeHtml(layoutCase.alt)}" src="assets/${layoutCase.id}-approved.svg" />
            </figure>
            <figure>
              <figcaption>Current engine</figcaption>
              <img alt="Current engine: ${escapeHtml(layoutCase.alt)}" src="assets/${layoutCase.id}-current.svg" />
            </figure>
          </div>
          <footer>
            <span>${quantity(layoutCase.expected.nodes, "node")} · ${quantity(layoutCase.expected.groups, "group")} · ${quantity(layoutCase.expected.edges, "edge")}</span>
            <span>${performanceText}</span>
          </footer>
          <details>
            <summary>Review source</summary>
            <pre><code>${escapeHtml(layoutCase.sourceText)}</code></pre>
          </details>
        </article>`;
    })
    .join("");

  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <meta name="description" content="Approved and current Stack Engine layout snapshots compared under identical versioned inputs." />
    <link rel="icon" href="data:," />
    <title>Stack layout regression gallery</title>
    <style>
      :root { color-scheme: light dark; font-family: ui-sans-serif, system-ui, sans-serif; }
      * { box-sizing: border-box; }
      body { margin: 0; background: Canvas; color: CanvasText; }
      main { width: min(1440px, 100%); margin: 0 auto; padding: clamp(20px, 4vw, 56px); }
      h1, h2, p { margin-top: 0; }
      h1 { max-width: 18ch; font-size: clamp(2rem, 6vw, 4.5rem); letter-spacing: -0.05em; line-height: 0.95; }
      .intro { max-width: 70ch; color: GrayText; }
      .summary { display: flex; flex-wrap: wrap; gap: 8px; margin: 24px 0 40px; }
      .summary span, .features li { border: 1px solid color-mix(in srgb, CanvasText 22%, transparent); border-radius: 999px; padding: 5px 10px; font: 600 0.72rem/1 ui-monospace, monospace; }
      .case { margin: 0 0 32px; border: 1px solid color-mix(in srgb, CanvasText 20%, transparent); border-radius: 18px; overflow: clip; }
      .case.changed { border-color: #d97706; }
      .case > header, .case > footer, details { padding: 20px; }
      .case > header { display: flex; justify-content: space-between; gap: 24px; border-bottom: 1px solid color-mix(in srgb, CanvasText 16%, transparent); }
      .case h2 { margin-bottom: 8px; font-size: 1.4rem; }
      .case header p { margin-bottom: 0; max-width: 68ch; color: GrayText; }
      .eyebrow { font: 600 0.7rem/1 ui-monospace, monospace; text-transform: uppercase; letter-spacing: 0.08em; }
      .result { flex: 0 0 auto; font-weight: 700; }
      .matches .result { color: #166534; }
      .changed .result { color: #9a3412; }
      .features { display: flex; flex-wrap: wrap; gap: 6px; margin: 0; padding: 16px 20px; list-style: none; }
      .comparison { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); border-block: 1px solid color-mix(in srgb, CanvasText 16%, transparent); }
      figure { min-width: 0; margin: 0; padding: 16px; background: color-mix(in srgb, CanvasText 3%, Canvas); }
      figure + figure { border-left: 1px solid color-mix(in srgb, CanvasText 16%, transparent); }
      figcaption { margin-bottom: 12px; font: 700 0.75rem/1 ui-monospace, monospace; text-transform: uppercase; letter-spacing: 0.08em; }
      img { display: block; width: 100%; height: min(62vh, 720px); object-fit: contain; background: white; border-radius: 10px; }
      .case > footer { display: flex; justify-content: space-between; gap: 16px; font: 600 0.78rem/1.4 ui-monospace, monospace; }
      details { border-top: 1px solid color-mix(in srgb, CanvasText 16%, transparent); }
      summary { cursor: pointer; font-weight: 700; }
      pre { overflow: auto; margin: 16px 0 0; padding: 16px; border-radius: 10px; background: color-mix(in srgb, CanvasText 8%, Canvas); font-size: 0.78rem; }
      @media (max-width: 760px) {
        .case > header, .case > footer { display: block; }
        .result, .case > footer span { display: block; margin-top: 12px; }
        .comparison { grid-template-columns: 1fr; }
        figure + figure { border-left: 0; border-top: 1px solid color-mix(in srgb, CanvasText 16%, transparent); }
        img { height: min(58vh, 560px); }
      }
      @media (prefers-color-scheme: dark) {
        .matches .result { color: #86efac; }
        .changed .result { color: #fdba74; }
      }
      @media (prefers-reduced-motion: reduce) { * { scroll-behavior: auto !important; } }
    </style>
  </head>
  <body>
    <main>
      <p class="eyebrow">Engine ${escapeHtml(catalog.engineVersion)} · corpus ${escapeHtml(catalog.schemaVersion)}</p>
      <h1>Layout regression gallery</h1>
      <p class="intro">Approved snapshots and current-engine candidates are rendered from the same versioned Stack sources. Exact SVG comparison catches every geometry change; this page makes an intentional change reviewable before the approved references move.</p>
      <div class="summary" aria-label="Gallery summary">
        <span>${catalog.cases.length} representative cases</span>
        <span>${changed} changed candidates</span>
        <span>${performance.suiteMilliseconds.toFixed(3)} ms benchmark suite</span>
        <span>${performance.maxP95Milliseconds.toFixed(3)} ms p95 budget</span>
      </div>
      ${cards}
    </main>
  </body>
</html>
`;
}

export async function buildLayoutGallery() {
  const { catalog, corpusRoot } = await validateRepositoryLayoutCorpus();
  const candidateRoot = path.join(repositoryRoot, "target/layout-corpus/candidate");
  const performance = JSON.parse(
    await readFile(path.join(repositoryRoot, "target/layout-corpus/performance.json"), "utf8"),
  );
  validatePerformance(catalog, performance);
  const performanceById = new Map(performance.cases.map((entry) => [entry.id, entry]));

  const comparisons = [];
  for (const layoutCase of catalog.cases) {
    const [approved, current, sourceText] = await Promise.all([
      readFile(path.join(corpusRoot, layoutCase.snapshot), "utf8"),
      readFile(path.join(candidateRoot, `${layoutCase.id}.svg`), "utf8"),
      readFile(path.join(corpusRoot, layoutCase.source), "utf8"),
    ]);
    comparisons.push({
      layoutCase: { ...layoutCase, sourceText },
      dimensions: svgDimensions(current),
      matches: approved === current,
      performanceCase: performanceById.get(layoutCase.id),
      approvedPath: path.join(corpusRoot, layoutCase.snapshot),
      currentPath: path.join(candidateRoot, `${layoutCase.id}.svg`),
    });
  }

  const targetRoot = path.join(repositoryRoot, "target");
  await mkdir(targetRoot, { recursive: true });
  const stagingRoot = await mkdtemp(path.join(targetRoot, ".layout-gallery-"));
  const assetRoot = path.join(stagingRoot, "assets");
  await mkdir(assetRoot);
  for (const comparison of comparisons) {
    await Promise.all([
      copyFile(
        comparison.approvedPath,
        path.join(assetRoot, `${comparison.layoutCase.id}-approved.svg`),
      ),
      copyFile(
        comparison.currentPath,
        path.join(assetRoot, `${comparison.layoutCase.id}-current.svg`),
      ),
    ]);
  }
  const html = renderGallery(catalog, comparisons, performance);
  assert.doesNotMatch(
    html,
    /<script\b|(?:src|href)="https?:\/\//i,
    "gallery must be static and local-only",
  );
  await writeFile(path.join(stagingRoot, "index.html"), html);

  const outputRoot = path.join(targetRoot, "layout-gallery");
  await rm(outputRoot, { recursive: true, force: true });
  await rename(stagingRoot, outputRoot);
  const changed = comparisons.filter((comparison) => !comparison.matches).length;
  console.log(
    `Built ${outputRoot}/index.html with ${comparisons.length} approved/current comparisons and ${changed} geometry changes.`,
  );
  return { outputRoot, comparisons };
}

function validatePerformance(catalog, performance) {
  assert.equal(performance.schemaVersion, "1.0");
  assert.equal(performance.profile, "release");
  assert.equal(performance.warmupIterations, catalog.performance.warmupIterations);
  assert.equal(performance.measuredIterations, catalog.performance.measuredIterations);
  assert.equal(performance.maxP95Milliseconds, catalog.performance.maxP95Milliseconds);
  assert.equal(performance.maxSuiteMilliseconds, catalog.performance.maxSuiteMilliseconds);
  assert.ok(performance.suiteMilliseconds <= catalog.performance.maxSuiteMilliseconds);
  assert.deepEqual(
    performance.cases.map(({ id }) => id),
    catalog.cases.map(({ id }) => id),
    "performance case inventory drift",
  );
  for (const entry of performance.cases) {
    assert.ok(Number.isFinite(entry.p95Milliseconds) && entry.p95Milliseconds >= 0);
    assert.ok(entry.p95Milliseconds <= catalog.performance.maxP95Milliseconds);
  }
}

function svgDimensions(svg) {
  const match = svg.match(/<svg\b[^>]*\bwidth="([^"]+)"[^>]*\bheight="([^"]+)"/);
  assert.ok(match, "candidate SVG has no width and height");
  return `${match[1]} × ${match[2]}`;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function quantity(count, singular) {
  return `${count} ${count === 1 ? singular : `${singular}s`}`;
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : null;
if (invokedPath === import.meta.url) await buildLayoutGallery();

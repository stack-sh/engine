use std::collections::BTreeSet;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Deserialize;
use serde_json::json;
use stack_engine::{Engine, ProviderAsset, ProviderPack};

const REQUIRED_DENSITIES: [&str; 3] = ["small", "medium", "dense"];
const REQUIRED_FEATURES: [&str; 8] = [
    "groups",
    "nested-groups",
    "rank-constraints",
    "order-constraints",
    "cross-edges",
    "edge-labels",
    "long-labels",
    "provider-icons",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutCatalog {
    #[serde(rename = "$schema")]
    schema: String,
    schema_version: String,
    engine_version: String,
    performance: PerformanceBudget,
    cases: Vec<LayoutCase>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PerformanceBudget {
    warmup_iterations: usize,
    measured_iterations: usize,
    max_p95_milliseconds: f64,
    max_suite_milliseconds: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutCase {
    id: String,
    title: String,
    summary: String,
    density: String,
    source: String,
    snapshot: String,
    features: Vec<String>,
    provider_fixture: Option<String>,
    expected: ExpectedOutput,
    alt: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExpectedOutput {
    nodes: usize,
    groups: usize,
    edges: usize,
    provider_notices: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderPackInput {
    manifest: stack_theme::ProviderPack,
    assets: Vec<ProviderAssetInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderAssetInput {
    path: String,
    svg: String,
}

#[test]
fn layout_corpus_matches_approved_snapshots() -> Result<(), Box<dyn Error>> {
    let root = repository_root();
    let catalog = load_catalog(&root)?;
    validate_catalog(&catalog)?;

    let candidate_root = root.join("target/layout-corpus/candidate");
    if candidate_root.exists() {
        fs::remove_dir_all(&candidate_root)?;
    }
    fs::create_dir_all(&candidate_root)?;

    let update_snapshots =
        std::env::var_os("UPDATE_STACK_LAYOUT_SNAPSHOTS") == Some(OsString::from("1"));
    for case in &catalog.cases {
        let source = fs::read(root.join("layout-corpus").join(&case.source))?;
        let packs = load_provider_packs(&root, case.provider_fixture.as_deref())?;
        let engine = if packs.is_empty() {
            Engine::bundled()
        } else {
            Engine::with_provider_packs(&packs)?
        };
        let output = engine.render(&source)?;
        if !output.diagnostics.is_empty() {
            return Err(
                format!("{} produced diagnostics: {:?}", case.id, output.diagnostics).into(),
            );
        }
        if output.provider_notices.len() != case.expected.provider_notices {
            return Err(format!(
                "{} produced {} provider notices instead of {}",
                case.id,
                output.provider_notices.len(),
                case.expected.provider_notices
            )
            .into());
        }
        let svg = output
            .svg
            .ok_or_else(|| format!("{} produced no SVG", case.id))?;
        validate_svg(case, &svg)?;

        let candidate = candidate_root.join(format!("{}.svg", case.id));
        fs::write(&candidate, &svg)?;
        let approved = root.join("layout-corpus").join(&case.snapshot);
        if update_snapshots {
            if let Some(parent) = approved.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&approved, &svg)?;
        } else if fs::read_to_string(&approved)? != svg {
            return Err(format!(
                "{} differs from its approved snapshot; review target/layout-gallery before updating",
                case.id
            )
            .into());
        }
    }

    assert_inventory(&root, &catalog)?;
    Ok(())
}

#[test]
#[ignore = "run explicitly in release mode to enforce the layout runtime budget"]
fn layout_runtime_stays_within_budget() -> Result<(), Box<dyn Error>> {
    let root = repository_root();
    let catalog = load_catalog(&root)?;
    validate_catalog(&catalog)?;
    let budget = catalog.performance;
    let suite_start = Instant::now();
    let mut results = Vec::new();

    for case in &catalog.cases {
        let source = fs::read(root.join("layout-corpus").join(&case.source))?;
        let packs = load_provider_packs(&root, case.provider_fixture.as_deref())?;
        let engine = if packs.is_empty() {
            Engine::bundled()
        } else {
            Engine::with_provider_packs(&packs)?
        };
        for _ in 0..budget.warmup_iterations {
            require_render(&engine, &source, &case.id)?;
        }

        let mut durations = Vec::with_capacity(budget.measured_iterations);
        for _ in 0..budget.measured_iterations {
            let started = Instant::now();
            require_render(&engine, &source, &case.id)?;
            durations.push(started.elapsed().as_secs_f64() * 1000.0);
        }
        durations.sort_by(f64::total_cmp);
        let percentile_index = (durations.len() * 95).div_ceil(100) - 1;
        let p95_milliseconds = durations[percentile_index];
        if p95_milliseconds > budget.max_p95_milliseconds {
            return Err(format!(
                "{} p95 {:.3} ms exceeds {:.3} ms",
                case.id, p95_milliseconds, budget.max_p95_milliseconds
            )
            .into());
        }
        results.push(json!({
            "id": case.id,
            "p95Milliseconds": rounded_milliseconds(p95_milliseconds),
            "minimumMilliseconds": rounded_milliseconds(durations[0]),
            "maximumMilliseconds": rounded_milliseconds(durations[durations.len() - 1])
        }));
    }

    let suite_milliseconds = suite_start.elapsed().as_secs_f64() * 1000.0;
    if suite_milliseconds > budget.max_suite_milliseconds {
        return Err(format!(
            "layout corpus suite {:.3} ms exceeds {:.3} ms",
            suite_milliseconds, budget.max_suite_milliseconds
        )
        .into());
    }

    let report = json!({
        "schemaVersion": "1.0",
        "profile": "release",
        "warmupIterations": budget.warmup_iterations,
        "measuredIterations": budget.measured_iterations,
        "maxP95Milliseconds": budget.max_p95_milliseconds,
        "maxSuiteMilliseconds": budget.max_suite_milliseconds,
        "suiteMilliseconds": rounded_milliseconds(suite_milliseconds),
        "cases": results
    });
    let report_root = root.join("target/layout-corpus");
    fs::create_dir_all(&report_root)?;
    let mut document = serde_json::to_vec_pretty(&report)?;
    document.push(b'\n');
    fs::write(report_root.join("performance.json"), document)?;
    eprintln!(
        "layout corpus: {} cases, {:.3} ms suite, {:.3} ms p95 budget",
        catalog.cases.len(),
        suite_milliseconds,
        budget.max_p95_milliseconds
    );
    Ok(())
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_catalog(root: &Path) -> Result<LayoutCatalog, Box<dyn Error>> {
    let document = fs::read(root.join("layout-corpus/catalog.json"))?;
    Ok(serde_json::from_slice(&document)?)
}

fn validate_catalog(catalog: &LayoutCatalog) -> Result<(), Box<dyn Error>> {
    if catalog.schema != "./schema.json"
        || catalog.schema_version != "1.0"
        || catalog.engine_version != stack_engine::ENGINE_VERSION
        || catalog.cases.len() < 6
        || catalog.performance.warmup_iterations == 0
        || catalog.performance.measured_iterations < 5
        || catalog.performance.max_p95_milliseconds <= 0.0
        || catalog.performance.max_suite_milliseconds <= 0.0
    {
        return Err("layout corpus metadata is invalid or incompatible".into());
    }

    let mut ids = BTreeSet::new();
    let mut densities = BTreeSet::new();
    let mut features = BTreeSet::new();
    for case in &catalog.cases {
        if !ids.insert(case.id.as_str())
            || case.title.is_empty()
            || case.summary.is_empty()
            || case.alt.is_empty()
            || case.source != format!("sources/{}.stack", case.id)
            || case.snapshot != format!("snapshots/{}.svg", case.id)
            || case.features.is_empty()
            || case.provider_fixture.is_some()
                != case.features.iter().any(|item| item == "provider-icons")
        {
            return Err(format!("{} has invalid or duplicate catalog metadata", case.id).into());
        }
        densities.insert(case.density.as_str());
        features.extend(case.features.iter().map(String::as_str));
    }
    for required in REQUIRED_DENSITIES {
        if !densities.contains(required) {
            return Err(format!("layout corpus does not cover {required} density").into());
        }
    }
    for required in REQUIRED_FEATURES {
        if !features.contains(required) {
            return Err(format!("layout corpus does not cover {required}").into());
        }
    }
    Ok(())
}

fn load_provider_packs(
    root: &Path,
    fixture: Option<&str>,
) -> Result<Vec<ProviderPack>, Box<dyn Error>> {
    let Some(fixture) = fixture else {
        return Ok(Vec::new());
    };
    let document = fs::read(root.join("layout-corpus").join(fixture))?;
    let inputs: Vec<ProviderPackInput> = serde_json::from_slice(&document)?;
    inputs
        .into_iter()
        .map(|input| {
            ProviderPack::new(
                input.manifest,
                input
                    .assets
                    .into_iter()
                    .map(|asset| ProviderAsset::new(asset.path, asset.svg))
                    .collect(),
            )
            .map_err(|error| Box::new(error) as Box<dyn Error>)
        })
        .collect()
}

fn require_render(engine: &Engine<'_>, source: &[u8], case_id: &str) -> Result<(), Box<dyn Error>> {
    let output = engine.render(source)?;
    if !output.diagnostics.is_empty() || output.svg.is_none() {
        return Err(format!("{case_id} did not produce a clean SVG during benchmarking").into());
    }
    Ok(())
}

fn validate_svg(case: &LayoutCase, svg: &str) -> Result<(), Box<dyn Error>> {
    let document = roxmltree::Document::parse(svg)?;
    let root = document.root_element();
    if !root.has_tag_name("svg")
        || root.attribute("role") != Some("img")
        || root.attribute("aria-labelledby") != Some("stack-title stack-description")
        || !document
            .descendants()
            .any(|node| node.attribute("id") == Some("stack-title"))
        || !document
            .descendants()
            .any(|node| node.attribute("id") == Some("stack-description"))
    {
        return Err(format!("{} has an invalid accessible SVG root", case.id).into());
    }
    let view_box = root
        .attribute("viewBox")
        .ok_or_else(|| format!("{} has no viewBox", case.id))?
        .split_ascii_whitespace()
        .map(str::parse::<i64>)
        .collect::<Result<Vec<_>, _>>()?;
    if view_box.len() != 4
        || view_box[0] != 0
        || view_box[1] != 0
        || view_box[2] <= 0
        || view_box[3] <= 0
    {
        return Err(format!("{} has invalid positive geometry bounds", case.id).into());
    }

    let nodes = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("data-node-kind").is_some())
        .count();
    let groups = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node.attribute("data-stack-id").is_some()
                && node.attribute("data-node-kind").is_none()
        })
        .count();
    let edges = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("data-edge-kind").is_some())
        .count();
    if (nodes, groups, edges)
        != (
            case.expected.nodes,
            case.expected.groups,
            case.expected.edges,
        )
    {
        return Err(format!(
            "{} geometry inventory is ({nodes}, {groups}, {edges}) instead of ({}, {}, {})",
            case.id, case.expected.nodes, case.expected.groups, case.expected.edges
        )
        .into());
    }

    let mut identifiers = BTreeSet::new();
    for node in document
        .descendants()
        .filter(|node| node.attribute("data-stack-id").is_some())
    {
        let identifier = node
            .attribute("data-stack-id")
            .ok_or("filtered Stack identifier is absent")?;
        if !identifiers.insert(identifier) {
            return Err(format!("{} repeats Stack identifier {identifier}", case.id).into());
        }
    }
    let unsafe_attribute = document.descendants().any(|node| {
        node.attributes().any(|attribute| {
            attribute.name().to_ascii_lowercase().starts_with("on")
                || matches!(attribute.name(), "href" | "src")
        })
    });
    if document.descendants().any(|node| {
        matches!(
            node.tag_name().name(),
            "script" | "foreignObject" | "iframe" | "object" | "embed"
        )
    }) || unsafe_attribute
        || svg.contains("href=\"http")
        || svg.contains("src=\"http")
    {
        return Err(format!("{} contains active or external SVG content", case.id).into());
    }
    if case.provider_fixture.is_some() && !svg.contains("data-icon-id=\"example:storage\"") {
        return Err(format!("{} did not embed its caller-owned provider icon", case.id).into());
    }
    Ok(())
}

fn assert_inventory(root: &Path, catalog: &LayoutCatalog) -> Result<(), Box<dyn Error>> {
    let expected_sources = catalog
        .cases
        .iter()
        .map(|case| case.source.trim_start_matches("sources/").to_owned())
        .collect::<BTreeSet<_>>();
    let expected_snapshots = catalog
        .cases
        .iter()
        .map(|case| case.snapshot.trim_start_matches("snapshots/").to_owned())
        .collect::<BTreeSet<_>>();
    let actual_sources = file_inventory(&root.join("layout-corpus/sources"), "stack")?;
    let actual_snapshots = file_inventory(&root.join("layout-corpus/snapshots"), "svg")?;
    if actual_sources != expected_sources || actual_snapshots != expected_snapshots {
        return Err("layout corpus source or snapshot inventory has drifted".into());
    }
    Ok(())
}

fn file_inventory(root: &Path, extension: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut inventory = BTreeSet::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some(extension)
        {
            return Err(
                format!("{} has an unexpected corpus entry", entry.path().display()).into(),
            );
        }
        inventory.insert(entry.file_name().to_string_lossy().into_owned());
    }
    Ok(inventory)
}

fn rounded_milliseconds(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

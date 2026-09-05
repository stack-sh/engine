use std::error::Error;
use std::time::{Duration, Instant};

use serde::Deserialize;
use stack_engine::{Engine, ProviderAsset, ProviderPack, SourcePosition};

const WARMUP_ITERATIONS: usize = 5;
const MEASURED_ITERATIONS: usize = 100;
const MAX_P95: Duration = Duration::from_millis(20);
const MAX_SUITE: Duration = Duration::from_millis(500);

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
#[ignore = "run explicitly in release mode to enforce the editor latency budget"]
fn language_intelligence_runtime_stays_within_budget() -> Result<(), Box<dyn Error>> {
    let source = "stack 1.0 diagram \"Provider\" { node store \"Store\" { icon \"example:s\" } }";
    let cursor = source.find("example:s").ok_or("missing icon prefix")? + "example:s".len();
    let position = SourcePosition {
        byte_offset: cursor as u64,
        line: 1,
        column: (cursor + 1) as u64,
    };
    let inputs: Vec<ProviderPackInput> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/provider-pack-input.json"
    ))?;
    let packs = inputs
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
        })
        .collect::<Result<Vec<_>, _>>()?;
    let engine = Engine::with_provider_packs(&packs)?;

    for version in 0..WARMUP_ITERATIONS as u64 {
        require_language_result(&engine, source, version, position)?;
    }

    let suite_started = Instant::now();
    let mut durations = Vec::with_capacity(MEASURED_ITERATIONS);
    for version in 0..MEASURED_ITERATIONS as u64 {
        let started = Instant::now();
        require_language_result(&engine, source, version, position)?;
        durations.push(started.elapsed());
    }
    let suite = suite_started.elapsed();
    durations.sort();
    let percentile_index = (durations.len() * 95).div_ceil(100) - 1;
    let p95 = durations[percentile_index];
    if p95 > MAX_P95 {
        return Err(format!(
            "language intelligence p95 {:.3} ms exceeds {:.3} ms",
            p95.as_secs_f64() * 1_000.0,
            MAX_P95.as_secs_f64() * 1_000.0
        )
        .into());
    }
    if suite > MAX_SUITE {
        return Err(format!(
            "language intelligence suite {:.3} ms exceeds {:.3} ms",
            suite.as_secs_f64() * 1_000.0,
            MAX_SUITE.as_secs_f64() * 1_000.0
        )
        .into());
    }
    eprintln!(
        "language intelligence: {MEASURED_ITERATIONS} completion + hover pairs, {:.3} ms suite, {:.3} ms p95",
        suite.as_secs_f64() * 1_000.0,
        p95.as_secs_f64() * 1_000.0
    );
    Ok(())
}

fn require_language_result(
    engine: &Engine<'_>,
    source: &str,
    document_version: u64,
    position: SourcePosition,
) -> Result<(), Box<dyn Error>> {
    let completion = engine.completion(source, document_version, position)?;
    if completion.document_version != document_version
        || completion.items.len() != 1
        || completion.items[0].filter_text != "example:storage"
    {
        return Err("completion output changed during the latency measurement".into());
    }
    let hover = engine.hover(source, document_version, position)?;
    if hover.document_version != document_version {
        return Err("hover output changed during the latency measurement".into());
    }
    Ok(())
}

use std::error::Error;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use stack_engine_wasm::{CompletionResult, HoverResult, SourcePosition};

const CURSOR: &str = "<|>";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureCase {
    name: String,
    document_version: u64,
    source_with_cursor: String,
    provider_packs: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FixtureOutput {
    name: String,
    completion: CompletionResult,
    hover: HoverResult,
}

fn source_and_position(marked: &str) -> Result<(String, SourcePosition), Box<dyn Error>> {
    let marker = marked
        .find(CURSOR)
        .ok_or("fixture cursor marker is missing")?;
    if marked[marker + CURSOR.len()..].contains(CURSOR) {
        return Err("fixture contains more than one cursor marker".into());
    }
    let mut source = marked.to_owned();
    source.replace_range(marker..marker + CURSOR.len(), "");
    let mut line = 1_u64;
    let mut column = 1_u64;
    let mut characters = source[..marker].chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\r' && characters.peek() == Some(&'\n') {
            characters.next();
            line += 1;
            column = 1;
        } else if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    Ok((
        source,
        SourcePosition {
            byte_offset: marker as u64,
            line,
            column,
        },
    ))
}

fn main() -> Result<(), Box<dyn Error>> {
    let fixture_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: language-intelligence-parity <fixture-path> <provider-pack-path>")?;
    let provider_path = std::env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .ok_or("usage: language-intelligence-parity <fixture-path> <provider-pack-path>")?;
    let fixtures = serde_json::from_slice::<Vec<FixtureCase>>(&std::fs::read(fixture_path)?)?;
    let provider_packs = std::fs::read_to_string(provider_path)?;
    let outputs = fixtures
        .into_iter()
        .map(|fixture| {
            let (source, position) = source_and_position(&fixture.source_with_cursor)?;
            let completion = if fixture.provider_packs {
                stack_engine_wasm::completion_with_provider_packs_text(
                    &source,
                    fixture.document_version,
                    position,
                    &provider_packs,
                )?
            } else {
                stack_engine_wasm::completion_text(&source, fixture.document_version, position)?
            };
            Ok(FixtureOutput {
                name: fixture.name,
                completion,
                hover: stack_engine_wasm::hover_text(&source, fixture.document_version, position)?,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    println!("{}", serde_json::to_string(&outputs)?);
    Ok(())
}

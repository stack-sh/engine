use std::error::Error;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use stack_engine_wasm::{CheckResult, FormatResult, RenderResult};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureCase {
    name: String,
    input: FixtureInput,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum FixtureInput {
    String { value: String },
    Bytes { value: Vec<u8> },
}

impl FixtureInput {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::String { value } => value.as_bytes(),
            Self::Bytes { value } => value,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FixtureOutput {
    name: String,
    format: FormatResult,
    check: CheckResult,
    render: RenderResult,
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: native-parity <fixture-path>")?;
    let cases = serde_json::from_slice::<Vec<FixtureCase>>(&std::fs::read(path)?)?;
    let outputs = cases
        .into_iter()
        .map(|case| {
            let source = case.input.bytes();
            Ok(FixtureOutput {
                name: case.name,
                format: stack_engine_wasm::format_bytes(source)?,
                check: stack_engine_wasm::check_bytes(source)?,
                render: stack_engine_wasm::render_bytes(source)?,
            })
        })
        .collect::<Result<Vec<_>, stack_engine::OperationalError>>()?;
    println!("{}", serde_json::to_string(&outputs)?);
    Ok(())
}

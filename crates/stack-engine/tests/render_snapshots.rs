#![cfg(feature = "conformance")]

use std::error::Error;
use std::path::{Path, PathBuf};

use stack_engine::{Engine, Severity};

#[test]
fn canonical_valid_fixtures_match_standalone_svg_snapshots() -> Result<(), Box<dyn Error>> {
    let specification = specification_root()?;
    let cases = [
        ("default-normalization", Vec::<&str>::new()),
        ("complete-semantics", vec!["STK5001"]),
    ];
    for (case, expected_codes) in cases {
        let source = std::fs::read(
            specification
                .join("conformance/valid")
                .join(case)
                .join("source.stack"),
        )?;
        let output = Engine::bundled().render(&source)?;
        if output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
        {
            return Err(format!("{case} produced an error diagnostic").into());
        }
        assert_eq!(
            output
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            expected_codes
        );
        let svg = output.svg.ok_or("valid fixture produced no SVG")?;
        let snapshot = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/snapshots/render")
            .join(format!("{case}.svg"));
        if std::env::var_os("UPDATE_STACK_SNAPSHOTS").is_some() {
            std::fs::write(&snapshot, &svg)?;
        } else {
            assert_eq!(svg, std::fs::read_to_string(&snapshot)?);
        }
    }
    Ok(())
}

fn specification_root() -> Result<PathBuf, Box<dyn Error>> {
    let configured = std::env::var_os("STACK_SPECIFICATION_DIR")
        .ok_or("STACK_SPECIFICATION_DIR must point to stack-sh/specification")?;
    let root = Path::new(&configured);
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()).into());
    }
    Ok(root.to_path_buf())
}

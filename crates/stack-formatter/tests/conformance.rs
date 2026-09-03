use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use stack_compiler::ir;
use stack_formatter::{format, format_bytes};

#[test]
fn canonical_formatter_cases_match_exactly() -> Result<(), Box<dyn Error>> {
    let root = specification_root()?.join("conformance/formatter");
    let mut cases = fs::read_dir(&root)?.collect::<Result<Vec<_>, _>>()?;
    cases.sort_by_key(|entry| entry.file_name());
    if cases.is_empty() {
        return Err(format!("no formatter cases found in {}", root.display()).into());
    }

    for entry in cases {
        let case = entry.path();
        if !case.is_dir() {
            continue;
        }
        let input = fs::read(case.join("input.stack"))?;
        let expected = fs::read_to_string(case.join("expected.stack"))?;
        let expected_ir = read_json(&case.join("expected.ir.json"))?;

        let output = format_bytes(&input);
        if !output.diagnostics.is_empty() {
            return Err(format!(
                "{} produced diagnostics: {:?}",
                case.display(),
                output.diagnostics
            )
            .into());
        }
        if output.source.as_deref() != Some(expected.as_str()) {
            return Err(format!("{} did not match expected source", case.display()).into());
        }
        if format(&expected).source.as_deref() != Some(expected.as_str()) {
            return Err(format!("{} was not idempotent", case.display()).into());
        }

        let input_text = std::str::from_utf8(&input)?;
        let input_output = stack_compiler::compile(input_text);
        let Some(input_diagram) = input_output.diagram else {
            return Err(format!("{} input did not compile", case.display()).into());
        };
        let expected_output = stack_compiler::compile(&expected);
        let Some(expected_diagram) = expected_output.diagram else {
            return Err(format!("{} expected source did not compile", case.display()).into());
        };
        if diagram_json(&input_diagram) != expected_ir
            || diagram_json(&expected_diagram) != expected_ir
        {
            return Err(format!("{} did not match expected normalized IR", case.display()).into());
        }
    }

    Ok(())
}

fn read_json(path: &Path) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn diagram_json(diagram: &ir::Diagram) -> Value {
    json!({
        "schemaVersion": "1.0",
        "languageVersion": {
            "major": diagram.language_version.major,
            "minor": diagram.language_version.minor,
        },
        "title": diagram.title,
        "themeId": diagram.theme_id,
        "children": diagram.children.iter().map(element_json).collect::<Vec<_>>(),
        "nodes": diagram.nodes.iter().map(node_json).collect::<Vec<_>>(),
        "groups": diagram.groups.iter().map(group_json).collect::<Vec<_>>(),
        "edges": diagram.edges.iter().map(edge_json).collect::<Vec<_>>(),
        "layout": diagram.layout.as_ref().map(layout_json),
    })
}

fn element_json(element: &ir::ElementId) -> Value {
    match element {
        ir::ElementId::Node(id) => json!({ "type": "node", "id": id }),
        ir::ElementId::Group(id) => json!({ "type": "group", "id": id }),
    }
}

fn node_json(node: &ir::Node) -> Value {
    json!({
        "id": node.id,
        "label": node.label,
        "kind": node_kind_name(node.kind),
        "iconId": node.icon_id,
        "detail": node.detail,
        "parentGroupId": node.parent_group_id,
    })
}

fn node_kind_name(kind: ir::NodeKind) -> &'static str {
    match kind {
        ir::NodeKind::Actor => "actor",
        ir::NodeKind::Client => "client",
        ir::NodeKind::Service => "service",
        ir::NodeKind::Function => "function",
        ir::NodeKind::Worker => "worker",
        ir::NodeKind::Database => "database",
        ir::NodeKind::Cache => "cache",
        ir::NodeKind::Queue => "queue",
        ir::NodeKind::Storage => "storage",
        ir::NodeKind::External => "external",
    }
}

fn group_json(group: &ir::Group) -> Value {
    json!({
        "id": group.id,
        "label": group.label,
        "parentGroupId": group.parent_group_id,
        "children": group.children.iter().map(element_json).collect::<Vec<_>>(),
        "layout": group.layout.as_ref().map(layout_json),
    })
}

fn edge_json(edge: &ir::Edge) -> Value {
    json!({
        "from": edge.from,
        "to": edge.to,
        "direction": edge_direction_name(edge.direction),
        "kind": edge_kind_name(edge.kind),
        "label": edge.label,
    })
}

fn edge_direction_name(direction: ir::EdgeDirection) -> &'static str {
    match direction {
        ir::EdgeDirection::Forward => "forward",
        ir::EdgeDirection::Bidirectional => "bidirectional",
        ir::EdgeDirection::Association => "association",
    }
}

fn edge_kind_name(kind: ir::EdgeKind) -> &'static str {
    match kind {
        ir::EdgeKind::Flow => "flow",
        ir::EdgeKind::Request => "request",
        ir::EdgeKind::Event => "event",
        ir::EdgeKind::Data => "data",
        ir::EdgeKind::Dependency => "dependency",
    }
}

fn layout_json(layout: &ir::Layout) -> Value {
    json!({
        "direction": layout.direction.map(direction_name),
        "sameRanks": layout.same_ranks,
        "order": layout.order,
    })
}

fn direction_name(direction: ir::Direction) -> &'static str {
    match direction {
        ir::Direction::Right => "right",
        ir::Direction::Down => "down",
    }
}

#[test]
fn canonical_examples_are_idempotent_and_semantically_equal() -> Result<(), Box<dyn Error>> {
    let root = specification_root()?.join("examples");
    let mut examples = fs::read_dir(&root)?.collect::<Result<Vec<_>, _>>()?;
    examples.sort_by_key(|entry| entry.file_name());
    if examples.is_empty() {
        return Err(format!("no examples found in {}", root.display()).into());
    }

    for entry in examples {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("stack") {
            continue;
        }
        let source = fs::read_to_string(&path)?;
        let output = format(&source);
        if !output.diagnostics.is_empty() {
            return Err(format!("{} produced diagnostics", path.display()).into());
        }
        let Some(formatted) = output.source else {
            return Err(format!("{} did not produce formatted source", path.display()).into());
        };

        if format(&formatted).source.as_deref() != Some(formatted.as_str()) {
            return Err(format!("{} was not idempotent", path.display()).into());
        }
        if stack_compiler::compile(&source).diagram != stack_compiler::compile(&formatted).diagram {
            return Err(format!("{} did not preserve normalized IR", path.display()).into());
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

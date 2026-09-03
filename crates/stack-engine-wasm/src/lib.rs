//! Typed browser adapter for the pure Stack engine.
//!
//! The Rust helpers expose the exact serializable result model used by the
//! WebAssembly boundary. The browser exports are generated only for
//! `wasm32-unknown-unknown` and accept either a JavaScript string or
//! `Uint8Array` without introducing filesystem, network, DOM, or clock access.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::Serialize;
use stack_engine::{Engine, OperationResult};

#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Object, Reflect, TypeError, Uint8Array};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};

/// JavaScript-facing result of the format operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatResult {
    /// Canonical source, absent after encoding, lexical, or syntax failure.
    pub formatted_source: Option<String>,
    /// Ordered portable diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Engine and input provenance.
    pub metadata: EngineMetadata,
}

/// JavaScript-facing result of the check operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    /// Ordered portable diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Engine and input provenance.
    pub metadata: EngineMetadata,
}

/// JavaScript-facing result of the render operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResult {
    /// Standalone SVG, absent whenever an error diagnostic prevents rendering.
    pub svg: Option<String>,
    /// Ordered portable diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Engine and input provenance.
    pub metadata: EngineMetadata,
}

/// Version metadata attached to every operation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineMetadata {
    /// Semantic version of `stack-engine`.
    pub engine_version: String,
    /// Authored language version, absent when parsing cannot recover it.
    pub language_version: Option<LanguageVersion>,
    /// Semantic version of the theme catalog.
    pub theme_catalog_version: String,
    /// Content revision of the theme catalog and icon bytes.
    pub theme_catalog_revision: String,
}

/// Authored Stack language version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageVersion {
    /// Major language version.
    pub major: u32,
    /// Minor language version.
    pub minor: u32,
}

/// Portable diagnostic shared with native engine consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// Stable Stack diagnostic identifier.
    pub code: String,
    /// Whether the diagnostic prevents the requested artifact.
    pub severity: Severity,
    /// Human-readable diagnostic description.
    pub message: String,
    /// Primary end-exclusive source range.
    pub range: SourceRange,
    /// Ordered source values or constructs valid at the primary range.
    pub expected: Vec<String>,
    /// Optional corrective guidance.
    pub help: Option<String>,
    /// Other source locations involved in the diagnostic.
    pub related: Vec<RelatedInformation>,
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Prevents the requested artifact.
    Error,
    /// Preserves successful output while reporting actionable information.
    Warning,
}

/// Additional source context related to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedInformation {
    /// Description of the related source location.
    pub message: String,
    /// End-exclusive related source range.
    pub range: SourceRange,
}

/// End-exclusive source range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    /// Inclusive source position.
    pub start: SourcePosition,
    /// Exclusive source position.
    pub end: SourcePosition,
}

/// One-based line and column with a zero-based UTF-8 byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePosition {
    /// Zero-based UTF-8 byte offset.
    pub byte_offset: u64,
    /// One-based source line.
    pub line: u64,
    /// One-based Unicode scalar column.
    pub column: u64,
}

/// Formats caller-owned source bytes through the bundled pure engine.
pub fn format_bytes(source: &[u8]) -> OperationResult<FormatResult> {
    Engine::bundled().format(source).map(FormatResult::from)
}

/// Checks caller-owned source bytes through the bundled pure engine.
pub fn check_bytes(source: &[u8]) -> OperationResult<CheckResult> {
    Engine::bundled().check(source).map(CheckResult::from)
}

/// Renders caller-owned source bytes through the bundled pure engine.
pub fn render_bytes(source: &[u8]) -> OperationResult<RenderResult> {
    Engine::bundled().render(source).map(RenderResult::from)
}

impl From<stack_engine::FormatOutput> for FormatResult {
    fn from(output: stack_engine::FormatOutput) -> Self {
        Self {
            formatted_source: output.formatted_source,
            diagnostics: output
                .diagnostics
                .into_iter()
                .map(Diagnostic::from)
                .collect(),
            metadata: EngineMetadata::from(output.metadata),
        }
    }
}

impl From<stack_engine::CheckOutput> for CheckResult {
    fn from(output: stack_engine::CheckOutput) -> Self {
        Self {
            diagnostics: output
                .diagnostics
                .into_iter()
                .map(Diagnostic::from)
                .collect(),
            metadata: EngineMetadata::from(output.metadata),
        }
    }
}

impl From<stack_engine::RenderOutput> for RenderResult {
    fn from(output: stack_engine::RenderOutput) -> Self {
        Self {
            svg: output.svg,
            diagnostics: output
                .diagnostics
                .into_iter()
                .map(Diagnostic::from)
                .collect(),
            metadata: EngineMetadata::from(output.metadata),
        }
    }
}

impl From<stack_engine::EngineMetadata> for EngineMetadata {
    fn from(metadata: stack_engine::EngineMetadata) -> Self {
        Self {
            engine_version: metadata.engine_version,
            language_version: metadata.language_version.map(LanguageVersion::from),
            theme_catalog_version: metadata.theme_catalog_version,
            theme_catalog_revision: metadata.theme_catalog_revision,
        }
    }
}

impl From<stack_engine::LanguageVersion> for LanguageVersion {
    fn from(version: stack_engine::LanguageVersion) -> Self {
        Self {
            major: version.major,
            minor: version.minor,
        }
    }
}

impl From<stack_engine::Diagnostic> for Diagnostic {
    fn from(diagnostic: stack_engine::Diagnostic) -> Self {
        Self {
            code: diagnostic.code,
            severity: Severity::from(diagnostic.severity),
            message: diagnostic.message,
            range: SourceRange::from(diagnostic.range),
            expected: diagnostic.expected,
            help: diagnostic.help,
            related: diagnostic
                .related
                .into_iter()
                .map(RelatedInformation::from)
                .collect(),
        }
    }
}

impl From<stack_engine::Severity> for Severity {
    fn from(severity: stack_engine::Severity) -> Self {
        match severity {
            stack_engine::Severity::Error => Self::Error,
            stack_engine::Severity::Warning => Self::Warning,
        }
    }
}

impl From<stack_engine::RelatedInformation> for RelatedInformation {
    fn from(related: stack_engine::RelatedInformation) -> Self {
        Self {
            message: related.message,
            range: SourceRange::from(related.range),
        }
    }
}

impl From<stack_engine::SourceRange> for SourceRange {
    fn from(range: stack_engine::SourceRange) -> Self {
        Self {
            start: SourcePosition::from(range.start),
            end: SourcePosition::from(range.end),
        }
    }
}

impl From<stack_engine::SourcePosition> for SourcePosition {
    fn from(position: stack_engine::SourcePosition) -> Self {
        Self {
            byte_offset: position.byte_offset,
            line: position.line,
            column: position.column,
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_TYPES: &'static str = r#"
export type StackSource = string | Uint8Array;
export type Severity = "error" | "warning";

export interface SourcePosition {
  readonly byteOffset: number;
  readonly line: number;
  readonly column: number;
}

export interface SourceRange {
  readonly start: SourcePosition;
  readonly end: SourcePosition;
}

export interface RelatedInformation {
  readonly message: string;
  readonly range: SourceRange;
}

export interface Diagnostic {
  readonly code: string;
  readonly severity: Severity;
  readonly message: string;
  readonly range: SourceRange;
  readonly expected: readonly string[];
  readonly help: string | null;
  readonly related: readonly RelatedInformation[];
}

export interface LanguageVersion {
  readonly major: number;
  readonly minor: number;
}

export interface EngineMetadata {
  readonly engineVersion: string;
  readonly languageVersion: LanguageVersion | null;
  readonly themeCatalogVersion: string;
  readonly themeCatalogRevision: string;
}

export interface FormatResult {
  readonly formattedSource: string | null;
  readonly diagnostics: readonly Diagnostic[];
  readonly metadata: EngineMetadata;
}

export interface CheckResult {
  readonly diagnostics: readonly Diagnostic[];
  readonly metadata: EngineMetadata;
}

export interface RenderResult {
  readonly svg: string | null;
  readonly diagnostics: readonly Diagnostic[];
  readonly metadata: EngineMetadata;
}

export function format(source: StackSource): FormatResult;
export function check(source: StackSource): CheckResult;
export function render(source: StackSource): RenderResult;
"#;

#[cfg(target_arch = "wasm32")]
/// Formats a JavaScript string or `Uint8Array` into the typed browser result.
#[wasm_bindgen(js_name = format, skip_typescript)]
pub fn format_js(source: JsValue) -> Result<JsValue, JsValue> {
    format_bytes(&source_bytes(source)?)
        .map_err(operation_error)
        .and_then(format_to_js)
}

#[cfg(target_arch = "wasm32")]
/// Checks a JavaScript string or `Uint8Array` into the typed browser result.
#[wasm_bindgen(js_name = check, skip_typescript)]
pub fn check_js(source: JsValue) -> Result<JsValue, JsValue> {
    check_bytes(&source_bytes(source)?)
        .map_err(operation_error)
        .and_then(check_to_js)
}

#[cfg(target_arch = "wasm32")]
/// Renders a JavaScript string or `Uint8Array` into the typed browser result.
#[wasm_bindgen(js_name = render, skip_typescript)]
pub fn render_js(source: JsValue) -> Result<JsValue, JsValue> {
    render_bytes(&source_bytes(source)?)
        .map_err(operation_error)
        .and_then(render_to_js)
}

#[cfg(target_arch = "wasm32")]
fn source_bytes(source: JsValue) -> Result<Vec<u8>, JsValue> {
    if let Some(source) = source.as_string() {
        return Ok(source.into_bytes());
    }
    if source.is_instance_of::<Uint8Array>() {
        return Ok(Uint8Array::new(&source).to_vec());
    }
    Err(TypeError::new("Stack source must be a string or Uint8Array").into())
}

#[cfg(target_arch = "wasm32")]
fn operation_error(error: stack_engine::OperationalError) -> JsValue {
    js_sys::Error::new(&error.to_string()).into()
}

#[cfg(target_arch = "wasm32")]
fn format_to_js(result: FormatResult) -> Result<JsValue, JsValue> {
    let output = Object::new();
    set_optional_string(&output, "formattedSource", result.formatted_source)?;
    set(
        &output,
        "diagnostics",
        diagnostics_to_js(result.diagnostics)?,
    )?;
    set(&output, "metadata", metadata_to_js(result.metadata)?)?;
    Ok(output.into())
}

#[cfg(target_arch = "wasm32")]
fn check_to_js(result: CheckResult) -> Result<JsValue, JsValue> {
    let output = Object::new();
    set(
        &output,
        "diagnostics",
        diagnostics_to_js(result.diagnostics)?,
    )?;
    set(&output, "metadata", metadata_to_js(result.metadata)?)?;
    Ok(output.into())
}

#[cfg(target_arch = "wasm32")]
fn render_to_js(result: RenderResult) -> Result<JsValue, JsValue> {
    let output = Object::new();
    set_optional_string(&output, "svg", result.svg)?;
    set(
        &output,
        "diagnostics",
        diagnostics_to_js(result.diagnostics)?,
    )?;
    set(&output, "metadata", metadata_to_js(result.metadata)?)?;
    Ok(output.into())
}

#[cfg(target_arch = "wasm32")]
fn diagnostics_to_js(diagnostics: Vec<Diagnostic>) -> Result<JsValue, JsValue> {
    let output = Array::new();
    for diagnostic in diagnostics {
        output.push(&diagnostic_to_js(diagnostic)?);
    }
    Ok(output.into())
}

#[cfg(target_arch = "wasm32")]
fn diagnostic_to_js(diagnostic: Diagnostic) -> Result<JsValue, JsValue> {
    let output = Object::new();
    set(&output, "code", diagnostic.code.into())?;
    set(
        &output,
        "severity",
        match diagnostic.severity {
            Severity::Error => JsValue::from_str("error"),
            Severity::Warning => JsValue::from_str("warning"),
        },
    )?;
    set(&output, "message", diagnostic.message.into())?;
    set(&output, "range", range_to_js(diagnostic.range)?)?;
    let expected = Array::new();
    for value in diagnostic.expected {
        expected.push(&value.into());
    }
    set(&output, "expected", expected.into())?;
    set_optional_string(&output, "help", diagnostic.help)?;
    let related = Array::new();
    for information in diagnostic.related {
        let item = Object::new();
        set(&item, "message", information.message.into())?;
        set(&item, "range", range_to_js(information.range)?)?;
        related.push(&item);
    }
    set(&output, "related", related.into())?;
    Ok(output.into())
}

#[cfg(target_arch = "wasm32")]
fn metadata_to_js(metadata: EngineMetadata) -> Result<JsValue, JsValue> {
    let output = Object::new();
    set(&output, "engineVersion", metadata.engine_version.into())?;
    set(
        &output,
        "languageVersion",
        metadata
            .language_version
            .map_or(Ok(JsValue::NULL), |version| {
                let value = Object::new();
                set(&value, "major", JsValue::from(version.major))?;
                set(&value, "minor", JsValue::from(version.minor))?;
                Ok::<JsValue, JsValue>(value.into())
            })?,
    )?;
    set(
        &output,
        "themeCatalogVersion",
        metadata.theme_catalog_version.into(),
    )?;
    set(
        &output,
        "themeCatalogRevision",
        metadata.theme_catalog_revision.into(),
    )?;
    Ok(output.into())
}

#[cfg(target_arch = "wasm32")]
fn range_to_js(range: SourceRange) -> Result<JsValue, JsValue> {
    let output = Object::new();
    set(&output, "start", position_to_js(range.start)?)?;
    set(&output, "end", position_to_js(range.end)?)?;
    Ok(output.into())
}

#[cfg(target_arch = "wasm32")]
fn position_to_js(position: SourcePosition) -> Result<JsValue, JsValue> {
    let output = Object::new();
    set(
        &output,
        "byteOffset",
        JsValue::from_f64(position.byte_offset as f64),
    )?;
    set(&output, "line", JsValue::from_f64(position.line as f64))?;
    set(&output, "column", JsValue::from_f64(position.column as f64))?;
    Ok(output.into())
}

#[cfg(target_arch = "wasm32")]
fn set_optional_string(object: &Object, name: &str, value: Option<String>) -> Result<(), JsValue> {
    set(object, name, value.map_or(JsValue::NULL, JsValue::from))
}

#[cfg(target_arch = "wasm32")]
fn set(object: &Object, name: &str, value: JsValue) -> Result<(), JsValue> {
    Reflect::set(object, &JsValue::from_str(name), &value).map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{Diagnostic, Severity, check_bytes, format_bytes, render_bytes};

    #[test]
    fn native_results_keep_operation_shapes_and_invalid_utf8() -> Result<(), Box<dyn Error>> {
        let source = b"stack 1.0 diagram \"API\" { node api \"API\" }";
        let formatted = format_bytes(source)?;
        let checked = check_bytes(source)?;
        let rendered = render_bytes(source)?;
        assert!(formatted.formatted_source.is_some());
        assert!(checked.diagnostics.is_empty());
        assert!(rendered.svg.is_some());
        assert_eq!(formatted.metadata, checked.metadata);
        assert_eq!(checked.metadata, rendered.metadata);

        for diagnostics in [
            format_bytes(b"\xff")?.diagnostics,
            check_bytes(b"\xff")?.diagnostics,
            render_bytes(b"\xff")?.diagnostics,
        ] {
            assert_eq!(diagnostics[0].code, "STK1001");
            assert_eq!(diagnostics[0].severity, Severity::Error);
        }
        Ok(())
    }

    #[test]
    fn conversion_keeps_warning_help_and_related_information() {
        let range = stack_engine::SourceRange {
            start: stack_engine::SourcePosition {
                byte_offset: 1,
                line: 1,
                column: 2,
            },
            end: stack_engine::SourcePosition {
                byte_offset: 4,
                line: 1,
                column: 5,
            },
        };
        let converted = Diagnostic::from(stack_engine::Diagnostic {
            code: "STK5001".to_owned(),
            severity: stack_engine::Severity::Warning,
            message: "fallback used".to_owned(),
            range,
            expected: vec!["available-resource".to_owned()],
            help: Some("install the resource".to_owned()),
            related: vec![stack_engine::RelatedInformation {
                message: "requested here".to_owned(),
                range,
            }],
        });
        assert_eq!(converted.severity, Severity::Warning);
        assert_eq!(converted.expected, ["available-resource"]);
        assert_eq!(converted.help.as_deref(), Some("install the resource"));
        assert_eq!(converted.related[0].message, "requested here");
        assert_eq!(converted.related[0].range.start.byte_offset, 1);
    }
}

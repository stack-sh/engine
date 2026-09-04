//! Pure operation facade and output contract for Stack diagrams.
//!
//! The facade accepts source bytes and a validated, versioned theme catalog. It
//! never reads the filesystem, network, process environment, clock, locale, or
//! host font APIs. Invalid user source is returned as ordered diagnostics in a
//! successful operation result; [`OperationalError`] is reserved for failures
//! in supplied execution inputs or violated internal pipeline invariants.
//!
//! ```
//! use stack_engine::Engine;
//!
//! let engine = Engine::bundled();
//! let output = engine.check(b"stack 1.0 diagram \"API\" { node api \"API\" }")?;
//! assert!(output.diagnostics.is_empty());
//! assert_eq!(output.metadata.language_version.map(|version| version.major), Some(1));
//! # Ok::<(), stack_engine::OperationalError>(())
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::error::Error;
use std::fmt;

use stack_compiler::diagnostic as compiler_diagnostic;

mod resources;
mod routing;
mod scene;
mod svg;

/// Version of the Rust engine facade.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Result channel for failures outside user-authored Stack source.
pub type OperationResult<T> = Result<T, OperationalError>;

/// Pure engine facade backed by one validated catalog and revision.
#[derive(Debug, Clone, Copy)]
pub struct Engine<'catalog> {
    catalog: &'catalog stack_theme::Catalog,
    catalog_revision: &'catalog str,
}

#[derive(Debug)]
struct PreparedScene<'catalog> {
    scene: scene::Scene,
    resources: resources::Resources<'catalog>,
    diagnostics: Vec<Diagnostic>,
}

impl Engine<'static> {
    /// Creates an engine backed by the catalog embedded in `stack-theme`.
    #[must_use]
    pub fn bundled() -> Self {
        Self {
            catalog: stack_theme::catalog(),
            catalog_revision: stack_theme::CATALOG_REVISION,
        }
    }
}

impl Default for Engine<'static> {
    fn default() -> Self {
        Self::bundled()
    }
}

impl<'catalog> Engine<'catalog> {
    /// Creates an engine from a previously validated catalog and its content revision.
    ///
    /// The catalog document must already have passed the public `stack-theme`
    /// schema and asset validator. This constructor checks the fallback records
    /// needed by every engine operation and rejects invalid execution input as an
    /// operational error rather than a user-source diagnostic.
    pub fn with_catalog(
        catalog: &'catalog stack_theme::Catalog,
        catalog_revision: &'catalog str,
    ) -> OperationResult<Self> {
        if !valid_catalog_revision(catalog_revision) {
            return Err(OperationalError::InvalidCatalog {
                reason: "catalog revision must be a lowercase sha256 digest",
            });
        }
        if !catalog
            .themes
            .iter()
            .any(|theme| theme.id == catalog.fallbacks.missing_theme_id)
        {
            return Err(OperationalError::InvalidCatalog {
                reason: "missing-theme fallback does not reference an active theme",
            });
        }
        if catalog.themes.iter().any(|theme| {
            !theme
                .icons
                .iter()
                .any(|icon| icon.id == catalog.fallbacks.missing_icon_id)
        }) {
            return Err(OperationalError::InvalidCatalog {
                reason: "missing-icon fallback is not present in every theme",
            });
        }

        Ok(Self {
            catalog,
            catalog_revision,
        })
    }

    /// Formats source while preserving compiler diagnostics in authored order.
    pub fn format(&self, source: &[u8]) -> OperationResult<FormatOutput> {
        let formatted = stack_formatter::format_bytes(source);
        Ok(FormatOutput {
            formatted_source: formatted.source,
            diagnostics: portable_diagnostics(formatted.diagnostics),
            metadata: self.metadata(declared_language_version(source)),
        })
    }

    /// Runs compiler, theme, layout, and routing validation without producing SVG.
    pub fn check(&self, source: &[u8]) -> OperationResult<CheckOutput> {
        let compiled = stack_compiler::compile_bytes_with_source_map(source);
        let mut diagnostics = portable_diagnostics(compiled.diagnostics);
        if let Some(diagram) = &compiled.diagram {
            let source_map = compiled.source_map.as_ref().ok_or(
                OperationalError::InvalidIntermediateRepresentation {
                    reason: "compiler omitted the source map for normalized IR",
                },
            )?;
            diagnostics.extend(self.prepare_scene(diagram, source_map)?.diagnostics);
        }
        Ok(CheckOutput {
            diagnostics,
            metadata: self.metadata(declared_language_version(source)),
        })
    }

    /// Produces a deterministic standalone SVG from valid Stack source.
    ///
    /// Invalid source returns a normal [`RenderOutput`] with ordered diagnostics
    /// and no SVG. Resource and layout warnings preserve a fallback SVG, while
    /// invalid catalog or intermediate pipeline state uses [`OperationalError`].
    pub fn render(&self, source: &[u8]) -> OperationResult<RenderOutput> {
        let compiled = stack_compiler::compile_bytes_with_source_map(source);
        let metadata = self.metadata(declared_language_version(source));
        if compiled.diagram.is_none() {
            return Ok(RenderOutput {
                svg: None,
                diagnostics: portable_diagnostics(compiled.diagnostics),
                metadata,
            });
        }

        let diagram = compiled.diagram.as_ref().ok_or(
            OperationalError::InvalidIntermediateRepresentation {
                reason: "compiler omitted normalized IR after successful compilation",
            },
        )?;
        let source_map = compiled.source_map.as_ref().ok_or(
            OperationalError::InvalidIntermediateRepresentation {
                reason: "compiler omitted the source map for normalized IR",
            },
        )?;
        let prepared = self.prepare_scene(diagram, source_map)?;
        let mut diagnostics = portable_diagnostics(compiled.diagnostics);
        diagnostics.extend(prepared.diagnostics);
        let svg = svg::render(diagram, &prepared.scene, &prepared.resources, &metadata).map_err(
            |error| OperationalError::InvalidIntermediateRepresentation {
                reason: error.reason(),
            },
        )?;
        Ok(RenderOutput {
            svg: Some(svg),
            diagnostics,
            metadata,
        })
    }

    fn metadata(&self, language_version: Option<LanguageVersion>) -> EngineMetadata {
        EngineMetadata {
            engine_version: ENGINE_VERSION.to_owned(),
            language_version,
            theme_catalog_version: self.catalog.catalog_version.clone(),
            theme_catalog_revision: self.catalog_revision.to_owned(),
        }
    }

    fn prepare_scene(
        &self,
        diagram: &stack_compiler::ir::Diagram,
        source_map: &stack_compiler::source_map::SourceMap,
    ) -> OperationResult<PreparedScene<'catalog>> {
        let resources = resources::Resources::resolve(diagram, self.catalog).map_err(|error| {
            OperationalError::InvalidCatalog {
                reason: error.reason(),
            }
        })?;
        let scene = scene::layout(diagram, self.catalog).map_err(|error| {
            OperationalError::InvalidIntermediateRepresentation {
                reason: error.reason(),
            }
        })?;
        if !scene.geometry_is_valid() {
            return Err(OperationalError::InvalidIntermediateRepresentation {
                reason: "layout produced invalid containment or overlap geometry",
            });
        }
        let mut diagnostics = resources
            .warnings
            .iter()
            .map(|warning| resource_diagnostic(warning, source_map))
            .collect::<OperationResult<Vec<_>>>()?;
        diagnostics.extend(
            scene
                .unsatisfied_orders
                .iter()
                .map(|scope| order_diagnostic(scope, source_map))
                .collect::<OperationResult<Vec<_>>>()?,
        );
        Ok(PreparedScene {
            scene,
            resources,
            diagnostics,
        })
    }
}

fn resource_diagnostic(
    warning: &resources::ResourceWarning,
    source_map: &stack_compiler::source_map::SourceMap,
) -> OperationResult<Diagnostic> {
    let (code, message, help, origin) = match warning {
        resources::ResourceWarning::MissingTheme(identifier) => (
            "STK6001",
            format!("theme '{identifier}' is unavailable; default theme was used"),
            "Install the requested theme or select an available theme.",
            source_map.theme(),
        ),
        resources::ResourceWarning::MissingIcon { node_id, icon_id } => (
            "STK5001",
            format!("icon '{icon_id}' is unavailable; the missing-icon fallback was used"),
            "Install the icon in the effective theme or remove the icon property.",
            source_map.node_icon(node_id).ok_or(
                OperationalError::InvalidIntermediateRepresentation {
                    reason: "source map omitted a normalized node",
                },
            )?,
        ),
    };
    let span = origin
        .span()
        .ok_or(OperationalError::InvalidIntermediateRepresentation {
            reason: "source map omitted an authored resource identifier",
        })?;
    Ok(Diagnostic {
        code: code.to_owned(),
        severity: Severity::Warning,
        message,
        range: SourceRange::from(span),
        expected: Vec::new(),
        help: Some(help.to_owned()),
        related: Vec::new(),
    })
}

fn order_diagnostic(
    scope: &scene::SceneScope,
    source_map: &stack_compiler::source_map::SourceMap,
) -> OperationResult<Diagnostic> {
    let origin = match scope {
        scene::SceneScope::Diagram => source_map.diagram_order(),
        scene::SceneScope::Group(identifier) => source_map.group_order(identifier).ok_or(
            OperationalError::InvalidIntermediateRepresentation {
                reason: "source map omitted a normalized group",
            },
        )?,
    };
    let span = origin
        .span()
        .ok_or(OperationalError::InvalidIntermediateRepresentation {
            reason: "source map omitted an authored order hint",
        })?;
    Ok(Diagnostic {
        code: "STK4001".to_owned(),
        severity: Severity::Warning,
        message: "order hint could not be satisfied by deterministic layout".to_owned(),
        range: SourceRange::from(span),
        expected: Vec::new(),
        help: Some("Adjust the order hint or same-rank constraints.".to_owned()),
        related: Vec::new(),
    })
}

/// Failure in execution inputs or internal pipeline invariants, not in Stack source.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OperationalError {
    /// A provided catalog violates an invariant required by pure execution.
    InvalidCatalog {
        /// Stable explanation of the violated catalog invariant.
        reason: &'static str,
    },
    /// Compiler or layout data violates an invariant required by pure execution.
    InvalidIntermediateRepresentation {
        /// Stable explanation of the violated invariant.
        reason: &'static str,
    },
}

impl fmt::Display for OperationalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCatalog { reason } => write!(formatter, "invalid theme catalog: {reason}"),
            Self::InvalidIntermediateRepresentation { reason } => {
                write!(formatter, "invalid intermediate representation: {reason}")
            }
        }
    }
}

impl Error for OperationalError {}

/// Version metadata attached to every operation output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineMetadata {
    /// Semantic version of `stack-engine`.
    pub engine_version: String,
    /// Authored language version, absent when decoding or syntax parsing fails.
    pub language_version: Option<LanguageVersion>,
    /// Semantic version of the selected theme catalog.
    pub theme_catalog_version: String,
    /// Content revision of the selected theme catalog and icon bytes.
    pub theme_catalog_revision: String,
}

/// Authored Stack language version when decoding and syntax parsing succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageVersion {
    /// Authored major language version.
    pub major: u32,
    /// Authored minor language version.
    pub minor: u32,
}

/// Result of the format operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOutput {
    /// Canonical source, absent after encoding, lexical, or syntax failure.
    pub formatted_source: Option<String>,
    /// Compiler diagnostics in deterministic authored order.
    pub diagnostics: Vec<Diagnostic>,
    /// Versions that identify the exact operation implementation and inputs.
    pub metadata: EngineMetadata,
}

/// Result of the check operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutput {
    /// Compiler, theme, and layout diagnostics in deterministic order.
    pub diagnostics: Vec<Diagnostic>,
    /// Versions that identify the exact operation implementation and inputs.
    pub metadata: EngineMetadata,
}

/// Result of the render operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutput {
    /// Standalone SVG, absent whenever an error diagnostic prevents rendering.
    pub svg: Option<String>,
    /// Compiler, theme, and layout diagnostics in deterministic order.
    pub diagnostics: Vec<Diagnostic>,
    /// Versions that identify the exact operation implementation and inputs.
    pub metadata: EngineMetadata,
}

/// Engine-owned portable diagnostic shared by native and future WASM outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable Stack diagnostic identifier.
    pub code: String,
    /// Whether the diagnostic prevents an artifact from being produced.
    pub severity: Severity,
    /// Concise human-readable diagnostic description.
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

/// Severity of one portable diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Prevents normalized or rendered output as defined by the operation.
    Error,
    /// Preserves successful output while reporting actionable information.
    Warning,
}

/// Additional source context related to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedInformation {
    /// Description of the related source location.
    pub message: String,
    /// End-exclusive related source range.
    pub range: SourceRange,
}

/// End-exclusive source range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRange {
    /// Inclusive source position.
    pub start: SourcePosition,
    /// Exclusive source position.
    pub end: SourcePosition,
}

/// One-based line and column with a zero-based UTF-8 byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    /// Zero-based UTF-8 byte offset.
    pub byte_offset: u64,
    /// One-based source line.
    pub line: u64,
    /// One-based Unicode scalar column.
    pub column: u64,
}

fn valid_catalog_revision(revision: &str) -> bool {
    revision.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn declared_language_version(source: &[u8]) -> Option<LanguageVersion> {
    stack_compiler::parse_bytes(source)
        .document
        .map(|document| LanguageVersion {
            major: document.version.major,
            minor: document.version.minor,
        })
}

fn portable_diagnostics(diagnostics: Vec<compiler_diagnostic::Diagnostic>) -> Vec<Diagnostic> {
    diagnostics.into_iter().map(Diagnostic::from).collect()
}

impl From<compiler_diagnostic::Diagnostic> for Diagnostic {
    fn from(diagnostic: compiler_diagnostic::Diagnostic) -> Self {
        Self {
            code: diagnostic.code.to_owned(),
            severity: match diagnostic.severity {
                compiler_diagnostic::Severity::Error => Severity::Error,
                compiler_diagnostic::Severity::Warning => Severity::Warning,
            },
            message: diagnostic.message,
            range: SourceRange::from(diagnostic.span),
            expected: diagnostic.expected,
            help: diagnostic.help,
            related: diagnostic
                .related
                .into_iter()
                .map(|related| RelatedInformation {
                    message: related.message,
                    range: SourceRange::from(related.span),
                })
                .collect(),
        }
    }
}

impl From<compiler_diagnostic::Span> for SourceRange {
    fn from(span: compiler_diagnostic::Span) -> Self {
        Self {
            start: SourcePosition::from(span.start),
            end: SourcePosition::from(span.end),
        }
    }
}

impl From<compiler_diagnostic::SourcePosition> for SourcePosition {
    fn from(position: compiler_diagnostic::SourcePosition) -> Self {
        Self {
            byte_offset: position.byte_offset as u64,
            line: position.line as u64,
            column: position.column as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use stack_compiler::diagnostic as compiler_diagnostic;

    use super::{
        Diagnostic, ENGINE_VERSION, Engine, LanguageVersion, OperationalError, Severity,
        SourcePosition,
    };

    const VALID_SOURCE: &[u8] = b"stack 1.0 diagram \"API\" { node api \"API\" }";

    #[test]
    fn bundled_engine_reports_all_version_metadata() {
        let engine = Engine::bundled();
        let result = engine.check(VALID_SOURCE);
        assert!(result.is_ok());
        if let Ok(output) = result {
            assert!(output.diagnostics.is_empty());
            assert_eq!(output.metadata.engine_version, ENGINE_VERSION);
            assert_eq!(
                output.metadata.language_version,
                Some(LanguageVersion { major: 1, minor: 0 })
            );
            assert_eq!(output.metadata.theme_catalog_version, "0.3.0");
            assert_eq!(
                output.metadata.theme_catalog_revision,
                stack_theme::CATALOG_REVISION
            );
            assert_eq!(Engine::default().check(VALID_SOURCE), Ok(output));
        }
    }

    #[test]
    fn bundled_catalog_resolves_explicit_core_icons() -> Result<(), Box<dyn Error>> {
        let expected_icons = [
            ("api", "Application programming interface"),
            ("web", "Web application"),
            ("mobile", "Mobile application"),
            ("desktop", "Desktop application"),
            ("server", "Server host"),
            ("container", "Application container"),
            ("cluster", "Compute cluster"),
            ("cloud", "Cloud environment"),
            ("scheduler", "Scheduled execution"),
            ("webhook", "Webhook endpoint"),
            ("identity", "Identity and access"),
            ("observability", "Observability system"),
        ];
        let catalog = stack_theme::catalog();
        assert_eq!(catalog.catalog_version, "0.3.0");
        assert_eq!(
            stack_theme::CATALOG_REVISION,
            "sha256:e4eaad0813fcfef4a203e861909ff38833270646f9097155974c7c92108c5b1e"
        );
        for theme in &catalog.themes {
            for (identifier, subject) in expected_icons {
                let icon = theme
                    .icons
                    .iter()
                    .find(|icon| icon.id == identifier)
                    .ok_or("core icon is unavailable in a bundled theme")?;
                assert_eq!(icon.subject, subject);
                assert_eq!(icon.asset.path, format!("assets/core/{identifier}.svg"));
            }
        }

        let source = b"stack 1.0 diagram \"Core icon\" { theme dark node gateway \"Gateway\" { kind service detail \"Public API\" icon \"api\" } }";
        let checked = Engine::bundled().check(source)?;
        let rendered = Engine::bundled().render(source)?;
        assert!(checked.diagnostics.is_empty());
        assert!(rendered.diagnostics.is_empty());
        assert_eq!(rendered.metadata.theme_catalog_version, "0.3.0");
        assert_eq!(
            rendered.metadata.theme_catalog_revision,
            stack_theme::CATALOG_REVISION
        );
        let svg = rendered.svg.ok_or("explicit icon render produced no SVG")?;
        assert!(svg.contains("data-icon-id=\"api\""));
        assert!(!svg.contains("data-icon-id=\"kind-external\""));
        Ok(())
    }

    #[test]
    fn format_preserves_semantic_diagnostics_but_not_syntax_failures() {
        let engine = Engine::bundled();
        let semantic_error = b"stack 1.0 diagram \"API\" { node api \"A\" node api \"B\" }";
        let semantic_result = engine.format(semantic_error);
        assert!(semantic_result.is_ok());
        if let Ok(semantic) = semantic_result {
            assert!(semantic.formatted_source.is_some());
            assert!(!semantic.diagnostics.is_empty());
        }

        let encoding_result = engine.format(b"stack 1.0\n\xff");
        assert!(encoding_result.is_ok());
        if let Ok(encoding) = encoding_result {
            assert!(encoding.formatted_source.is_none());
            assert_eq!(encoding.diagnostics[0].code, "STK1001");
            assert_eq!(encoding.metadata.language_version, None);
        }
    }

    #[test]
    fn check_keeps_compiler_diagnostic_order_and_positions() {
        let source =
            b"stack 1.0 diagram \"API\" { node api \"A\" node api \"B\" edge api -> missing }";
        let expected = stack_compiler::compile_bytes(source)
            .diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();
        let result = Engine::bundled().check(source);
        assert!(result.is_ok());
        if let Ok(output) = result {
            assert_eq!(
                output
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(
                output
                    .diagnostics
                    .windows(2)
                    .all(|pair| pair[0].range.start.byte_offset <= pair[1].range.start.byte_offset)
            );
        }
    }

    #[test]
    fn check_emits_order_warning_at_the_authored_statement() -> Result<(), Box<dyn Error>> {
        let source = "stack 1.0 diagram \"Order\" { layout { direction right order [b, a] } node a \"A\" node b \"B\" }";
        let output = Engine::bundled().check(source.as_bytes())?;
        assert_eq!(output.diagnostics.len(), 1);
        let diagnostic = &output.diagnostics[0];
        assert_eq!(diagnostic.code, "STK4001");
        assert_eq!(diagnostic.severity, Severity::Warning);
        let start = source
            .find("order [b, a]")
            .ok_or("missing order statement")?;
        let end = start + "order [b, a]".len();
        assert_eq!(diagnostic.range.start.byte_offset, start as u64);
        assert_eq!(diagnostic.range.end.byte_offset, end as u64);
        assert_eq!(diagnostic.range.start.line, 1);
        assert_eq!(diagnostic.range.start.column, start as u64 + 1);
        assert_eq!(diagnostic.range.end.column, end as u64 + 1);
        Ok(())
    }

    #[test]
    fn check_omits_order_warning_when_rank_placement_satisfies_it() -> Result<(), Box<dyn Error>> {
        let source = b"stack 1.0 diagram \"Order\" { layout { direction right rank same [a, b] order [b, a] } node a \"A\" node b \"B\" }";
        let output = Engine::bundled().check(source)?;
        assert!(output.diagnostics.is_empty());
        Ok(())
    }

    #[test]
    fn group_order_warning_uses_the_group_source_map_entry() -> Result<(), Box<dyn Error>> {
        let source = "stack 1.0 diagram \"Group order\" { group pair \"Pair\" { layout { direction down order [b, a] } node a \"A\" node b \"B\" } }";
        let output = Engine::bundled().check(source.as_bytes())?;
        assert_eq!(output, Engine::bundled().check(source.as_bytes())?);
        assert_eq!(
            output
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            vec!["STK4001"]
        );
        let start = source
            .find("order [b, a]")
            .ok_or("missing order statement")?;
        assert_eq!(output.diagnostics[0].range.start.byte_offset, start as u64);
        Ok(())
    }

    #[test]
    fn layout_warnings_follow_compiler_warnings() -> Result<(), Box<dyn Error>> {
        let mut source = String::from(
            "stack 1.0 diagram \"Warnings\" { layout { direction right order [n1, n0] } node hub \"Hub\" ",
        );
        for index in 0..13 {
            source.push_str(&format!(
                "node n{index} \"N {index}\" edge hub -> n{index} "
            ));
        }
        source.push('}');
        let output = Engine::bundled().check(source.as_bytes())?;
        assert_eq!(
            output
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            vec!["STK4002", "STK4001"]
        );
        Ok(())
    }

    #[test]
    fn resource_fallbacks_report_authored_ranges_and_render_svg() -> Result<(), Box<dyn Error>> {
        let source = "stack 1.0 diagram \"Fallbacks\" { theme neon layout { direction right order [b, a] } node a \"A\" { icon \"missing\" } node b \"B\" }";
        let checked = Engine::bundled().check(source.as_bytes())?;
        let rendered = Engine::bundled().render(source.as_bytes())?;
        assert_eq!(checked.diagnostics, rendered.diagnostics);
        assert_eq!(
            rendered
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>(),
            vec!["STK6001", "STK5001", "STK4001"]
        );

        let theme_start = source.find("neon").ok_or("missing theme identifier")?;
        assert_eq!(
            rendered.diagnostics[0].range.start.byte_offset,
            theme_start as u64
        );
        assert_eq!(
            rendered.diagnostics[0].range.end.byte_offset,
            (theme_start + "neon".len()) as u64
        );
        let icon_start = source.find("\"missing\"").ok_or("missing icon string")?;
        assert_eq!(
            rendered.diagnostics[1].range.start.byte_offset,
            icon_start as u64
        );
        assert_eq!(
            rendered.diagnostics[1].range.end.byte_offset,
            (icon_start + "\"missing\"".len()) as u64
        );
        let svg = rendered.svg.ok_or("render produced no SVG")?;
        assert!(svg.contains("data-theme-id=\"default\""));
        assert!(svg.contains("data-icon-id=\"kind-external\""));
        Ok(())
    }

    #[test]
    fn render_is_repeatable_and_escapes_source_text() -> Result<(), Box<dyn Error>> {
        let source = b"stack 1.0 diagram \"<script>&\" { node client \"\\\" onload=\\\"alert(1)<&>\" edge client -> api \"javascript:alert(1)\" node api \"API\" }";
        let first = Engine::bundled().render(source)?;
        let second = Engine::bundled().render(source)?;
        assert_eq!(first, second);
        let svg = first.svg.ok_or("render produced no SVG")?;
        assert!(svg.contains("&lt;script&gt;&amp;"));
        assert!(svg.contains("&quot; onload=&quot;alert(1)&lt;&amp;&gt;"));
        assert!(!svg.contains("<script"));
        assert!(!svg.contains("href="));
        Ok(())
    }

    #[cfg(feature = "conformance")]
    #[test]
    fn canonical_valid_fixtures_render_standalone_svg() -> Result<(), Box<dyn Error>> {
        let specification = std::env::var("STACK_SPECIFICATION_DIR")?;
        let valid_root = std::path::Path::new(&specification).join("conformance/valid");
        let mut cases = std::fs::read_dir(&valid_root)?.collect::<Result<Vec<_>, _>>()?;
        cases.sort_by_key(|entry| entry.file_name());
        if cases.is_empty() {
            return Err(format!("no valid fixtures found in {}", valid_root.display()).into());
        }

        for case in cases {
            let source_path = case.path().join("source.stack");
            if !source_path.is_file() {
                continue;
            }
            let output = Engine::bundled().render(&std::fs::read(&source_path)?)?;
            if output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error)
            {
                return Err(
                    format!("{} produced an error diagnostic", source_path.display()).into(),
                );
            }
            let svg = output
                .svg
                .ok_or_else(|| format!("{} produced no standalone SVG", source_path.display()))?;
            assert!(svg.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
            assert!(svg.ends_with("</svg>\n"));
        }
        Ok(())
    }

    #[test]
    fn render_separates_invalid_input_from_success() -> Result<(), Box<dyn Error>> {
        let engine = Engine::bundled();
        let result = engine.render(b"\xff");
        assert!(result.is_ok());
        if let Ok(output) = result {
            assert!(output.svg.is_none());
            assert_eq!(output.diagnostics[0].code, "STK1001");
            assert_eq!(output.metadata.language_version, None);
        }

        let output = engine.render(VALID_SOURCE)?;
        assert!(output.diagnostics.is_empty());
        assert!(
            output
                .svg
                .as_deref()
                .is_some_and(|svg| svg.contains("<svg"))
        );
        Ok(())
    }

    #[test]
    fn provided_catalog_requires_usable_fallbacks_and_revision() {
        let catalog = stack_theme::catalog().clone();
        assert!(Engine::with_catalog(&catalog, stack_theme::CATALOG_REVISION).is_ok());
        assert!(matches!(
            Engine::with_catalog(&catalog, "sha256:NOT-A-DIGEST"),
            Err(OperationalError::InvalidCatalog { .. })
        ));

        let mut missing_theme = catalog.clone();
        missing_theme.fallbacks.missing_theme_id = "missing".to_owned();
        assert!(matches!(
            Engine::with_catalog(&missing_theme, stack_theme::CATALOG_REVISION),
            Err(OperationalError::InvalidCatalog { .. })
        ));

        let mut missing_icon = catalog;
        missing_icon.fallbacks.missing_icon_id = "missing".to_owned();
        assert!(matches!(
            Engine::with_catalog(&missing_icon, stack_theme::CATALOG_REVISION),
            Err(OperationalError::InvalidCatalog { .. })
        ));
    }

    #[test]
    fn diagnostic_conversion_keeps_expected_help_and_related_ranges() {
        let start = compiler_diagnostic::SourcePosition {
            byte_offset: 3,
            line: 2,
            column: 4,
        };
        let end = compiler_diagnostic::SourcePosition {
            byte_offset: 7,
            line: 2,
            column: 8,
        };
        let diagnostic = compiler_diagnostic::Diagnostic {
            code: "STK4002",
            severity: compiler_diagnostic::Severity::Warning,
            message: "warning".to_owned(),
            span: compiler_diagnostic::Span { start, end },
            expected: vec!["right".to_owned(), "down".to_owned()],
            help: Some("help".to_owned()),
            related: vec![compiler_diagnostic::RelatedInformation {
                message: "related".to_owned(),
                span: compiler_diagnostic::Span::point(start),
            }],
        };

        let portable = Diagnostic::from(diagnostic);
        assert_eq!(portable.severity, Severity::Warning);
        assert_eq!(portable.expected, ["right", "down"]);
        assert_eq!(portable.help.as_deref(), Some("help"));
        assert_eq!(portable.related[0].message, "related");
        assert_eq!(
            portable.range.start,
            SourcePosition {
                byte_offset: 3,
                line: 2,
                column: 4,
            }
        );
    }

    #[test]
    fn operational_error_messages_are_stable() {
        assert_eq!(
            OperationalError::InvalidCatalog { reason: "reason" }.to_string(),
            "invalid theme catalog: reason"
        );
        assert_eq!(
            OperationalError::InvalidIntermediateRepresentation { reason: "reason" }.to_string(),
            "invalid intermediate representation: reason"
        );
    }
}

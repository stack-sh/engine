//! Theme-aware adapter for protocol-neutral compiler language intelligence.

use std::collections::BTreeMap;

use stack_compiler::{
    diagnostic as compiler_diagnostic, language_intelligence as compiler_language,
};

use crate::{Diagnostic, Engine, OperationResult, OperationalError, SourcePosition, SourceRange};

/// Portable language-intelligence schema version implemented by the pinned compiler.
pub const LANGUAGE_INTELLIGENCE_SCHEMA_VERSION: &str = compiler_language::SCHEMA_VERSION;

impl Engine<'_> {
    /// Computes context-aware completion from one complete UTF-8 source snapshot.
    ///
    /// The response echoes `document_version` so a host can discard stale work.
    /// Core and caller-owned provider icons come from the same validated catalogs
    /// used by check and render operations.
    pub fn completion(
        &self,
        source: &str,
        document_version: u64,
        position: SourcePosition,
    ) -> OperationResult<CompletionOutput> {
        let catalog = self.completion_catalog()?;
        compiler_language::completion(
            source,
            document_version,
            compiler_diagnostic::SourcePosition::try_from(position)?,
            &catalog,
        )
        .map(CompletionOutput::from)
        .map_err(language_intelligence_error)
    }

    /// Resolves plain-text semantic hover for one complete UTF-8 source snapshot.
    ///
    /// The response echoes `document_version` so a host can discard stale work.
    pub fn hover(
        &self,
        source: &str,
        document_version: u64,
        position: SourcePosition,
    ) -> OperationResult<HoverOutput> {
        compiler_language::hover(
            source,
            document_version,
            compiler_diagnostic::SourcePosition::try_from(position)?,
        )
        .map(HoverOutput::from)
        .map_err(language_intelligence_error)
    }

    fn completion_catalog(&self) -> OperationResult<compiler_language::CompletionCatalog> {
        let mut icons = BTreeMap::new();
        for theme in &self.catalog.themes {
            for icon in &theme.icons {
                icons.entry(icon.id.clone()).or_insert_with(|| {
                    compiler_language::CompletionCatalogEntry {
                        id: icon.id.clone(),
                        label: icon.id.clone(),
                        detail: Some(icon.subject.clone()),
                        documentation: icon.description.clone(),
                    }
                });
            }
        }
        for pack in self.provider_packs {
            let manifest = pack.manifest();
            for icon in &manifest.icons {
                icons.insert(
                    icon.id.clone(),
                    compiler_language::CompletionCatalogEntry {
                        id: icon.id.clone(),
                        label: icon.id.clone(),
                        detail: Some(icon.product_name.clone()),
                        documentation: Some(format!(
                            "{} provider icon: {}",
                            manifest.provider.name, icon.subject
                        )),
                    },
                );
            }
        }
        if icons.len() > compiler_language::MAX_COMPLETION_ICONS {
            return Err(OperationalError::InvalidLanguageIntelligenceInput {
                reason: "completion catalog exceeds the item limit",
            });
        }
        Ok(compiler_language::CompletionCatalog {
            icons: icons.into_values().collect(),
        })
    }
}

fn language_intelligence_error(error: compiler_language::IntelligenceError) -> OperationalError {
    let reason = match error {
        compiler_language::IntelligenceError::InvalidPosition => "source position is invalid",
        compiler_language::IntelligenceError::CompletionCatalogTooLarge => {
            "completion catalog exceeds the item limit"
        }
        compiler_language::IntelligenceError::InvalidCompletionCatalogEntry { .. } => {
            "completion catalog contains an invalid entry"
        }
        compiler_language::IntelligenceError::DuplicateCompletionCatalogId { .. } => {
            "completion catalog contains a duplicate icon id"
        }
    };
    OperationalError::InvalidLanguageIntelligenceInput { reason }
}

impl TryFrom<SourcePosition> for compiler_diagnostic::SourcePosition {
    type Error = OperationalError;

    fn try_from(position: SourcePosition) -> Result<Self, Self::Error> {
        Ok(Self {
            byte_offset: usize::try_from(position.byte_offset).map_err(|_| {
                OperationalError::InvalidLanguageIntelligenceInput {
                    reason: "source position exceeds the target address space",
                }
            })?,
            line: usize::try_from(position.line).map_err(|_| {
                OperationalError::InvalidLanguageIntelligenceInput {
                    reason: "source position exceeds the target address space",
                }
            })?,
            column: usize::try_from(position.column).map_err(|_| {
                OperationalError::InvalidLanguageIntelligenceInput {
                    reason: "source position exceeds the target address space",
                }
            })?,
        })
    }
}

/// A source replacement interpreted against the unchanged input snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// End-exclusive source range replaced by this edit.
    pub range: SourceRange,
    /// Literal Stack source inserted in place of the range.
    pub new_text: String,
}

/// Semantic category of one completion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// A grammatical Stack keyword.
    Keyword,
    /// A property or layout statement valid in the current block.
    Property,
    /// A closed value from the Stack language specification.
    EnumValue,
    /// A document-local semantic identifier.
    Identifier,
    /// An icon from the engine's core or caller-owned provider catalog.
    Icon,
}

/// One literal, protocol-neutral source completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    /// User-visible plain-text label.
    pub label: String,
    /// Semantic completion category.
    pub kind: CompletionKind,
    /// Optional plain-text secondary label.
    pub detail: Option<String>,
    /// Optional plain-text documentation.
    pub documentation: Option<String>,
    /// Plain string used by consumers for filtering.
    pub filter_text: String,
    /// Stable ordering key.
    pub sort_text: String,
    /// Literal source replacement for this item.
    pub edit: TextEdit,
}

/// Completion result for one caller-owned document version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionOutput {
    /// Portable language-intelligence schema version.
    pub schema_version: String,
    /// Document version supplied by the caller.
    pub document_version: u64,
    /// Ordered compiler diagnostics for the same source snapshot.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether more source context may materially change the list.
    pub is_incomplete: bool,
    /// Deterministically ordered completion items.
    pub items: Vec<CompletionItem>,
}

/// Semantic category described by hover information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverKind {
    /// The document's diagram declaration.
    Diagram,
    /// A containment group.
    Group,
    /// A node declaration or reference.
    Node,
    /// An edge declaration.
    Edge,
    /// A language property, theme, or layout value.
    Property,
}

/// Plain-text semantic information for one source token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    /// Exact source range described by this hover.
    pub range: SourceRange,
    /// Semantic category.
    pub kind: HoverKind,
    /// Short user-visible label.
    pub label: String,
    /// Optional plain-text secondary label.
    pub detail: Option<String>,
    /// Optional plain-text documentation.
    pub documentation: Option<String>,
}

/// Hover result for one caller-owned document version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverOutput {
    /// Portable language-intelligence schema version.
    pub schema_version: String,
    /// Document version supplied by the caller.
    pub document_version: u64,
    /// Ordered compiler diagnostics for the same source snapshot.
    pub diagnostics: Vec<Diagnostic>,
    /// Resolved semantic hover, if a trustworthy construct covers the position.
    pub hover: Option<Hover>,
}

impl From<compiler_language::CompletionOutput> for CompletionOutput {
    fn from(output: compiler_language::CompletionOutput) -> Self {
        Self {
            schema_version: output.schema_version.to_owned(),
            document_version: output.document_version,
            diagnostics: output
                .diagnostics
                .into_iter()
                .map(Diagnostic::from)
                .collect(),
            is_incomplete: output.is_incomplete,
            items: output.items.into_iter().map(CompletionItem::from).collect(),
        }
    }
}

impl From<compiler_language::CompletionItem> for CompletionItem {
    fn from(item: compiler_language::CompletionItem) -> Self {
        Self {
            label: item.label,
            kind: CompletionKind::from(item.kind),
            detail: item.detail,
            documentation: item.documentation,
            filter_text: item.filter_text,
            sort_text: item.sort_text,
            edit: TextEdit {
                range: SourceRange::from(item.edit.range),
                new_text: item.edit.new_text,
            },
        }
    }
}

impl From<compiler_language::CompletionKind> for CompletionKind {
    fn from(kind: compiler_language::CompletionKind) -> Self {
        match kind {
            compiler_language::CompletionKind::Keyword => Self::Keyword,
            compiler_language::CompletionKind::Property => Self::Property,
            compiler_language::CompletionKind::EnumValue => Self::EnumValue,
            compiler_language::CompletionKind::Identifier => Self::Identifier,
            compiler_language::CompletionKind::Icon => Self::Icon,
        }
    }
}

impl From<compiler_language::HoverOutput> for HoverOutput {
    fn from(output: compiler_language::HoverOutput) -> Self {
        Self {
            schema_version: output.schema_version.to_owned(),
            document_version: output.document_version,
            diagnostics: output
                .diagnostics
                .into_iter()
                .map(Diagnostic::from)
                .collect(),
            hover: output.hover.map(Hover::from),
        }
    }
}

impl From<compiler_language::Hover> for Hover {
    fn from(hover: compiler_language::Hover) -> Self {
        Self {
            range: SourceRange::from(hover.range),
            kind: HoverKind::from(hover.kind),
            label: hover.label,
            detail: hover.detail,
            documentation: hover.documentation,
        }
    }
}

impl From<compiler_language::HoverKind> for HoverKind {
    fn from(kind: compiler_language::HoverKind) -> Self {
        match kind {
            compiler_language::HoverKind::Diagram => Self::Diagram,
            compiler_language::HoverKind::Group => Self::Group,
            compiler_language::HoverKind::Node => Self::Node,
            compiler_language::HoverKind::Edge => Self::Edge,
            compiler_language::HoverKind::Property => Self::Property,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{CompletionKind, HoverKind, language_intelligence_error};
    use crate::{Engine, OperationalError, ProviderAsset, ProviderPack, SourcePosition};

    fn position(source: &str, byte_offset: usize) -> SourcePosition {
        let mut line = 1_u64;
        let mut column = 1_u64;
        for character in source[..byte_offset].chars() {
            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        SourcePosition {
            byte_offset: byte_offset as u64,
            line,
            column,
        }
    }

    #[test]
    fn completion_uses_context_and_echoes_the_document_version() -> Result<(), Box<dyn Error>> {
        let source = "stack 1.0\ndiagram \"Draft\" {\n  no\n}\n";
        let cursor = source.find("no").ok_or("missing prefix")? + 2;
        let output = Engine::bundled().completion(source, 42, position(source, cursor))?;
        assert_eq!(output.schema_version, "1.0");
        assert_eq!(output.document_version, 42);
        assert!(output.is_incomplete);
        assert_eq!(output.items.len(), 1);
        assert_eq!(output.items[0].label, "node");
        assert_eq!(output.items[0].kind, CompletionKind::Keyword);
        assert_eq!(output.items[0].edit.new_text, "node");
        Ok(())
    }

    #[test]
    fn completion_discovers_core_icon_ids() -> Result<(), Box<dyn Error>> {
        let source = "stack 1.0 diagram \"Icons\" { node api \"API\" { icon \"ga\" } }";
        let cursor = source.find("ga").ok_or("missing icon prefix")? + 2;
        let output = Engine::bundled().completion(source, 3, position(source, cursor))?;
        assert_eq!(output.items.len(), 1);
        let item = &output.items[0];
        assert_eq!(item.label, "gateway");
        assert_eq!(item.filter_text, "gateway");
        assert_eq!(item.kind, CompletionKind::Icon);
        assert_eq!(item.detail.as_deref(), Some("Network gateway"));
        assert_eq!(item.edit.new_text, "gateway");
        Ok(())
    }

    #[test]
    fn completion_includes_validated_provider_icons() -> Result<(), Box<dyn Error>> {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/provider-pack-input.json"
        ))?;
        let input = fixture
            .as_array()
            .and_then(|items| items.first())
            .ok_or("missing provider fixture")?;
        let manifest: stack_theme::ProviderPack =
            serde_json::from_value(input.get("manifest").cloned().ok_or("missing manifest")?)?;
        let assets = input
            .get("assets")
            .and_then(serde_json::Value::as_array)
            .ok_or("missing assets")?
            .iter()
            .map(|asset| {
                ProviderAsset::new(
                    asset
                        .get("path")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                    asset
                        .get("svg")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                )
            })
            .collect();
        let pack = ProviderPack::new(manifest, assets)?;
        let packs = [pack];
        let engine = Engine::with_provider_packs(&packs)?;
        let source =
            "stack 1.0 diagram \"Provider\" { node store \"Store\" { icon \"example:s\" } }";
        let cursor = source.find("example:s").ok_or("missing icon prefix")? + "example:s".len();
        let output = engine.completion(source, 9, position(source, cursor))?;
        assert_eq!(output.items.len(), 1);
        assert_eq!(output.items[0].label, "example:storage");
        assert_eq!(output.items[0].detail.as_deref(), Some("Example Storage"));
        assert_eq!(
            output.items[0].documentation.as_deref(),
            Some("Example Cloud provider icon: Object storage service")
        );
        Ok(())
    }

    #[test]
    fn hover_resolves_semantics_and_preserves_exact_ranges() -> Result<(), Box<dyn Error>> {
        let source = "stack 1.0 diagram \"API\" { node api \"Public API\" edge api -> client node client \"Client\" }";
        let reference = source.find("edge api").ok_or("missing edge")? + "edge ".len();
        let output = Engine::bundled().hover(source, 11, position(source, reference))?;
        assert_eq!(output.schema_version, "1.0");
        assert_eq!(output.document_version, 11);
        let hover = output.hover.ok_or("missing hover")?;
        assert_eq!(hover.kind, HoverKind::Node);
        assert_eq!(hover.label, "Public API");
        assert_eq!(hover.detail.as_deref(), Some("node api · service"));
        assert_eq!(hover.range.start.byte_offset, reference as u64);
        assert_eq!(hover.range.end.byte_offset, (reference + 3) as u64);
        Ok(())
    }

    #[test]
    fn invalid_position_uses_the_operational_error_channel() {
        let result = Engine::bundled().completion(
            "stack 1.0",
            1,
            SourcePosition {
                byte_offset: 4,
                line: 9,
                column: 9,
            },
        );
        assert!(matches!(
            result,
            Err(OperationalError::InvalidLanguageIntelligenceInput {
                reason: "source position is invalid"
            })
        ));
    }

    #[test]
    fn catalog_limits_and_validation_use_the_operational_error_channel()
    -> Result<(), Box<dyn Error>> {
        let source = "stack 1.0 diagram \"Icons\" { node api \"API\" { icon \"\" } }";
        let cursor = source.find("\"\"").ok_or("missing empty icon")? + 1;
        let mut oversized = stack_theme::catalog().clone();
        let template = oversized.themes[0].icons[0].clone();
        for index in 0..=stack_compiler::language_intelligence::MAX_COMPLETION_ICONS {
            let mut icon = template.clone();
            icon.id = format!("extra-{index}");
            oversized.themes[0].icons.push(icon);
        }
        let engine = Engine::with_catalog(&oversized, stack_theme::CATALOG_REVISION)?;
        assert!(matches!(
            engine.completion(source, 1, position(source, cursor)),
            Err(OperationalError::InvalidLanguageIntelligenceInput {
                reason: "completion catalog exceeds the item limit"
            })
        ));

        let mut invalid = stack_theme::catalog().clone();
        invalid.themes[0].icons[0].id = "INVALID".to_owned();
        let engine = Engine::with_catalog(&invalid, stack_theme::CATALOG_REVISION)?;
        assert!(matches!(
            engine.completion(source, 1, position(source, cursor)),
            Err(OperationalError::InvalidLanguageIntelligenceInput {
                reason: "completion catalog contains an invalid entry"
            })
        ));
        Ok(())
    }

    #[test]
    fn compiler_catalog_errors_have_stable_engine_messages() {
        use stack_compiler::language_intelligence::IntelligenceError;

        assert_eq!(
            language_intelligence_error(IntelligenceError::CompletionCatalogTooLarge).to_string(),
            "invalid language-intelligence input: completion catalog exceeds the item limit"
        );
        assert_eq!(
            language_intelligence_error(IntelligenceError::DuplicateCompletionCatalogId {
                index: 1,
            })
            .to_string(),
            "invalid language-intelligence input: completion catalog contains a duplicate icon id"
        );
    }
}

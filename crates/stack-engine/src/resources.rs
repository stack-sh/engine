//! Deterministic theme and icon resolution for scene validation and rendering.

use stack_compiler::ir::{Diagram, NodeKind};
use stack_theme::{Catalog, FontMetrics, Icon, NodeVisual, Theme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResourceWarning {
    MissingTheme(String),
    MissingIcon { node_id: String, icon_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourceError {
    reason: &'static str,
}

impl ResourceError {
    pub(crate) const fn reason(self) -> &'static str {
        self.reason
    }
}

#[derive(Debug)]
pub(crate) struct ResolvedNode<'catalog> {
    pub(crate) node_id: String,
    pub(crate) visual: &'catalog NodeVisual,
    pub(crate) icon: &'catalog Icon,
    pub(crate) icon_svg: &'static str,
}

#[derive(Debug)]
pub(crate) struct Resources<'catalog> {
    pub(crate) theme: &'catalog Theme,
    pub(crate) metrics: &'catalog FontMetrics,
    pub(crate) nodes: Vec<ResolvedNode<'catalog>>,
    pub(crate) warnings: Vec<ResourceWarning>,
}

impl<'catalog> Resources<'catalog> {
    pub(crate) fn resolve(
        diagram: &Diagram,
        catalog: &'catalog Catalog,
    ) -> Result<Self, ResourceError> {
        let requested_theme = catalog
            .themes
            .iter()
            .find(|theme| theme.id == diagram.theme_id);
        let (theme, mut warnings) = match requested_theme {
            Some(theme) => (theme, Vec::new()),
            None => (
                catalog
                    .themes
                    .iter()
                    .find(|theme| theme.id == catalog.fallbacks.missing_theme_id)
                    .ok_or(ResourceError {
                        reason: "missing-theme fallback is unavailable",
                    })?,
                vec![ResourceWarning::MissingTheme(diagram.theme_id.clone())],
            ),
        };
        let metrics = catalog
            .font_metrics
            .iter()
            .find(|metrics| metrics.id == theme.typography.font_metrics_id)
            .ok_or(ResourceError {
                reason: "resolved theme font metrics are unavailable",
            })?;

        let mut nodes = Vec::with_capacity(diagram.nodes.len());
        for node in &diagram.nodes {
            let visual = node_visual(theme, node.kind);
            let requested_icon = node.icon_id.as_deref().unwrap_or(&visual.fallback_icon_id);
            let icon = match theme.icons.iter().find(|icon| icon.id == requested_icon) {
                Some(icon) => icon,
                None if node.icon_id.is_some() => {
                    warnings.push(ResourceWarning::MissingIcon {
                        node_id: node.id.clone(),
                        icon_id: requested_icon.to_owned(),
                    });
                    theme
                        .icons
                        .iter()
                        .find(|icon| icon.id == catalog.fallbacks.missing_icon_id)
                        .ok_or(ResourceError {
                            reason: "missing-icon fallback is unavailable in the resolved theme",
                        })?
                }
                None => {
                    return Err(ResourceError {
                        reason: "node-kind fallback icon is unavailable in the resolved theme",
                    });
                }
            };
            let icon_svg = stack_theme::icon_svg(&icon.asset.path).ok_or(ResourceError {
                reason: "resolved icon bytes are not embedded in stack-theme",
            })?;
            nodes.push(ResolvedNode {
                node_id: node.id.clone(),
                visual,
                icon,
                icon_svg,
            });
        }

        Ok(Self {
            theme,
            metrics,
            nodes,
            warnings,
        })
    }

    pub(crate) fn node(&self, identifier: &str) -> Option<&ResolvedNode<'catalog>> {
        self.nodes.iter().find(|node| node.node_id == identifier)
    }
}

pub(crate) fn node_visual(theme: &Theme, kind: NodeKind) -> &NodeVisual {
    match kind {
        NodeKind::Actor => &theme.node_kind_fallbacks.actor,
        NodeKind::Client => &theme.node_kind_fallbacks.client,
        NodeKind::Service => &theme.node_kind_fallbacks.service,
        NodeKind::Function => &theme.node_kind_fallbacks.function_,
        NodeKind::Worker => &theme.node_kind_fallbacks.worker,
        NodeKind::Database => &theme.node_kind_fallbacks.database,
        NodeKind::Cache => &theme.node_kind_fallbacks.cache,
        NodeKind::Queue => &theme.node_kind_fallbacks.queue,
        NodeKind::Storage => &theme.node_kind_fallbacks.storage,
        NodeKind::External => &theme.node_kind_fallbacks.external,
    }
}

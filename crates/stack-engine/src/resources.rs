//! Deterministic theme and icon resolution for scene validation and rendering.

use stack_compiler::ir::{Diagram, NodeKind};
use stack_theme::{Catalog, FontMetrics, NodeVisual, ProviderIcon, Theme};

use crate::{ProviderNotice, ProviderNoticeIcon, ProviderNoticeSource, ProviderPack};

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
    pub(crate) icon_id: &'catalog str,
    pub(crate) icon_view_box: [i32; 4],
    pub(crate) icon_svg: &'catalog str,
    provider: Option<ResolvedProviderIcon<'catalog>>,
}

#[derive(Debug)]
struct ResolvedProviderIcon<'catalog> {
    pack: &'catalog ProviderPack,
    icon: &'catalog ProviderIcon,
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
        provider_packs: &'catalog [ProviderPack],
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
            if let Some((pack, icon, svg)) = provider_icon(provider_packs, requested_icon) {
                nodes.push(ResolvedNode {
                    node_id: node.id.clone(),
                    visual,
                    icon_id: &icon.id,
                    icon_view_box: icon.asset.view_box,
                    icon_svg: svg,
                    provider: Some(ResolvedProviderIcon { pack, icon }),
                });
                continue;
            }
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
                icon_id: &icon.id,
                icon_view_box: icon.asset.view_box,
                icon_svg,
                provider: None,
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

    pub(crate) fn provider_notices(&self) -> Vec<ProviderNotice> {
        let mut notices = Vec::new();
        for node in &self.nodes {
            let Some(resolved) = &node.provider else {
                continue;
            };
            let notice_index = notices
                .iter()
                .position(|notice: &ProviderNotice| {
                    notice.provider_id == resolved.pack.manifest().provider.id
                })
                .unwrap_or_else(|| {
                    let manifest = resolved.pack.manifest();
                    let mut sources = vec![ProviderNoticeSource {
                        id: "primary".to_owned(),
                        page_url: manifest.source.page_url.clone(),
                        release: manifest.source.release.clone(),
                        archive_sha256: manifest.source.archive_sha256.clone(),
                        terms_url: manifest.source.terms_url.clone(),
                    }];
                    sources.extend(manifest.additional_sources.iter().map(|additional| {
                        ProviderNoticeSource {
                            id: additional.id.clone(),
                            page_url: additional.source.page_url.clone(),
                            release: additional.source.release.clone(),
                            archive_sha256: additional.source.archive_sha256.clone(),
                            terms_url: additional.source.terms_url.clone(),
                        }
                    }));
                    notices.push(ProviderNotice {
                        provider_id: manifest.provider.id.clone(),
                        provider_name: manifest.provider.name.clone(),
                        pack_version: manifest.pack_version.clone(),
                        pack_revision: resolved.pack.revision().to_owned(),
                        source_release: manifest.source.release.clone(),
                        archive_sha256: manifest.source.archive_sha256.clone(),
                        terms_url: manifest.source.terms_url.clone(),
                        sources,
                        attribution: manifest.notice.attribution.clone(),
                        terms_summary: manifest.notice.terms_summary.clone(),
                        non_endorsement: manifest.notice.non_endorsement.clone(),
                        icons: Vec::new(),
                    });
                    notices.len() - 1
                });
            if !notices[notice_index]
                .icons
                .iter()
                .any(|icon| icon.id == resolved.icon.id)
            {
                notices[notice_index].icons.push(ProviderNoticeIcon {
                    id: resolved.icon.id.clone(),
                    product_name: resolved.icon.product_name.clone(),
                    brand_source_url: resolved.icon.brand_source_url.clone(),
                    brand_guidelines_url: resolved.icon.brand_guidelines_url.clone(),
                    source_id: resolved
                        .icon
                        .asset
                        .source_id
                        .clone()
                        .unwrap_or_else(|| "primary".to_owned()),
                });
            }
        }
        notices
    }
}

fn provider_icon<'catalog>(
    provider_packs: &'catalog [ProviderPack],
    identifier: &str,
) -> Option<(
    &'catalog ProviderPack,
    &'catalog ProviderIcon,
    &'catalog str,
)> {
    for pack in provider_packs {
        if let Some((icon, svg)) = pack.icon(identifier) {
            return Some((pack, icon, svg));
        }
    }
    None
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

//! Validated caller-owned provider packs for pure rendering.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use roxmltree::{Document, NodeType};
use sha2::{Digest, Sha256};
use stack_theme::{
    ProviderIcon, ProviderPack as ProviderPackManifest, ProviderPackPermittedOutput,
};

use crate::{OperationResult, OperationalError};

const MAX_PROVIDER_ASSET_BYTES: usize = 1024 * 1024;
const MAX_PROVIDER_PACK_BYTES: usize = 32 * 1024 * 1024;
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const ALLOWED_ELEMENTS: &[&str] = &[
    "circle",
    "defs",
    "ellipse",
    "g",
    "line",
    "linearGradient",
    "path",
    "polygon",
    "polyline",
    "radialGradient",
    "rect",
    "stop",
    "svg",
];
const ALLOWED_ATTRIBUTES: &[&str] = &[
    "aria-hidden",
    "clip-rule",
    "cx",
    "cy",
    "d",
    "fill",
    "fill-rule",
    "gradientTransform",
    "gradientUnits",
    "height",
    "id",
    "opacity",
    "offset",
    "points",
    "r",
    "role",
    "rx",
    "ry",
    "stop-color",
    "stop-opacity",
    "stroke",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-width",
    "transform",
    "viewBox",
    "width",
    "x",
    "x1",
    "x2",
    "y",
    "y1",
    "y2",
];

/// One caller-owned processed SVG addressed by its provider-manifest path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAsset {
    path: String,
    svg: String,
}

impl ProviderAsset {
    /// Creates one in-memory provider asset without reading host state.
    #[must_use]
    pub fn new(path: impl Into<String>, svg: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            svg: svg.into(),
        }
    }

    /// Returns the manifest-relative asset path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the complete processed SVG document.
    #[must_use]
    pub fn svg(&self) -> &str {
        &self.svg
    }
}

/// One validated, content-addressed provider pack held entirely in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPack {
    manifest: ProviderPackManifest,
    revision: String,
    assets: Vec<ProviderAsset>,
}

impl ProviderPack {
    /// Validates a typed manifest and its exact processed assets.
    ///
    /// The host remains responsible for JSON Schema validation, terms review,
    /// and local file selection. This constructor rechecks the invariants needed
    /// for safe deterministic embedding and computes a content revision without
    /// filesystem, network, clock, or environment access.
    pub fn new(
        manifest: ProviderPackManifest,
        assets: Vec<ProviderAsset>,
    ) -> OperationResult<Self> {
        validate_manifest_boundary(&manifest)?;
        validate_assets(&manifest, &assets)?;
        let revision = pack_revision(&manifest, &assets)?;
        Ok(Self {
            manifest,
            revision,
            assets,
        })
    }

    /// Returns the validated public provider-pack manifest.
    #[must_use]
    pub fn manifest(&self) -> &ProviderPackManifest {
        &self.manifest
    }

    /// Returns the deterministic manifest-and-assets SHA-256 revision.
    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub(crate) fn icon(&self, identifier: &str) -> Option<(&ProviderIcon, &str)> {
        let icon = self
            .manifest
            .icons
            .iter()
            .find(|icon| icon.id == identifier)?;
        let asset = self
            .assets
            .iter()
            .find(|asset| asset.path == icon.asset.path)?;
        Some((icon, &asset.svg))
    }
}

fn validate_manifest_boundary(manifest: &ProviderPackManifest) -> OperationResult<()> {
    let redistribution = &manifest.rights.redistribution;
    let processing = &manifest.rights.processing;
    if manifest.schema_version != "1.0"
        || manifest.icons.is_empty()
        || manifest.icons.len() > 10_000
        || !manifest.rights.terms_acceptance_required
        || !manifest
            .rights
            .permitted_outputs
            .contains(&ProviderPackPermittedOutput::ArchitectureDiagram)
        || redistribution.cargo
        || redistribution.npm
        || redistribution.wasm
        || redistribution.web_asset
        || redistribution.native_binary
        || !redistribution.generated_output
        || !processing.local_only
        || processing.automatic_download
        || processing.server_upload
        || !processing.preserve_colors
        || !processing.preserve_geometry
    {
        return Err(invalid_pack(
            "provider pack violates the user-imported rendering boundary",
        ));
    }
    if !valid_provider_id(&manifest.provider.id)
        || !valid_sha256(&manifest.source.archive_sha256)
        || manifest.source.terms_url.is_empty()
        || manifest.notice.attribution.is_empty()
        || manifest.notice.terms_summary.is_empty()
        || manifest.notice.non_endorsement.is_empty()
    {
        return Err(invalid_pack("provider pack identity or notice is invalid"));
    }

    let prefix = format!("{}:", manifest.provider.id);
    let mut identifiers = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for icon in &manifest.icons {
        if !icon.id.starts_with(&prefix)
            || !valid_namespaced_icon_id(&icon.id)
            || !identifiers.insert(icon.id.as_str())
            || !valid_asset_path(&icon.asset.path)
            || !paths.insert(icon.asset.path.as_str())
            || !valid_sha256(&icon.asset.original_sha256)
            || !valid_sha256(&icon.asset.processed_sha256)
            || icon.asset.view_box[2] <= 0
            || icon.asset.view_box[3] <= 0
        {
            return Err(invalid_pack(
                "provider pack contains an invalid or duplicate icon record",
            ));
        }
    }
    Ok(())
}

fn validate_assets(
    manifest: &ProviderPackManifest,
    assets: &[ProviderAsset],
) -> OperationResult<()> {
    if assets.len() != manifest.icons.len() {
        return Err(invalid_pack(
            "provider pack assets do not match the manifest",
        ));
    }
    let mut by_path = BTreeMap::new();
    let mut total_asset_bytes = 0usize;
    for asset in assets {
        total_asset_bytes = total_asset_bytes
            .checked_add(asset.svg.len())
            .ok_or_else(|| invalid_pack("provider pack assets exceed the total size limit"))?;
        if total_asset_bytes > MAX_PROVIDER_PACK_BYTES {
            return Err(invalid_pack(
                "provider pack assets exceed the total size limit",
            ));
        }
        if !valid_asset_path(&asset.path)
            || asset.svg.len() > MAX_PROVIDER_ASSET_BYTES
            || by_path
                .insert(asset.path.as_str(), asset.svg.as_str())
                .is_some()
        {
            return Err(invalid_pack(
                "provider pack contains an invalid or duplicate asset",
            ));
        }
    }
    for icon in &manifest.icons {
        let Some(svg) = by_path.get(icon.asset.path.as_str()) else {
            return Err(invalid_pack(
                "provider pack assets do not match the manifest",
            ));
        };
        if sha256(svg.as_bytes()) != icon.asset.processed_sha256 {
            return Err(invalid_pack(
                "provider pack asset hash does not match the manifest",
            ));
        }
        validate_svg(svg, icon.asset.view_box)?;
    }
    Ok(())
}

fn validate_svg(svg: &str, expected_view_box: [i32; 4]) -> OperationResult<()> {
    let uppercase = svg.to_ascii_uppercase();
    if uppercase.contains("<!DOCTYPE") || uppercase.contains("<!ENTITY") || svg.contains("<?") {
        return Err(unsafe_svg());
    }
    let document = Document::parse(svg).map_err(|_| unsafe_svg())?;
    let root = document.root_element();
    if root.tag_name().name() != "svg"
        || root.tag_name().namespace() != Some(SVG_NAMESPACE)
        || parse_view_box(root.attribute("viewBox")) != Some(expected_view_box)
    {
        return Err(unsafe_svg());
    }

    let mut declared = BTreeSet::new();
    let mut referenced = BTreeSet::new();
    for node in document.descendants() {
        match node.node_type() {
            NodeType::Root => continue,
            NodeType::Text if node.text().is_some_and(|text| text.trim().is_empty()) => continue,
            NodeType::Element => {}
            _ => return Err(unsafe_svg()),
        }
        let name = node.tag_name().name();
        let parent_name = node.parent_element().map(|parent| parent.tag_name().name());
        if !ALLOWED_ELEMENTS.contains(&name)
            || node.tag_name().namespace() != Some(SVG_NAMESPACE)
            || (name == "svg" && node != root)
            || (name == "defs" && parent_name != Some("svg"))
            || (matches!(name, "linearGradient" | "radialGradient") && parent_name != Some("defs"))
            || (name == "stop" && !matches!(parent_name, Some("linearGradient" | "radialGradient")))
            || (parent_name == Some("defs") && !matches!(name, "linearGradient" | "radialGradient"))
        {
            return Err(unsafe_svg());
        }
        for attribute in node.attributes() {
            let attribute_name = attribute.name();
            if attribute.namespace().is_some()
                || attribute_name.starts_with("on")
                || !ALLOWED_ATTRIBUTES.contains(&attribute_name)
            {
                return Err(unsafe_svg());
            }
            if attribute_name == "id"
                && (!matches!(name, "linearGradient" | "radialGradient")
                    || !attribute.value().starts_with("stack-")
                    || !declared.insert(attribute.value()))
            {
                return Err(unsafe_svg());
            }
            if let Some(identifier) = local_reference(attribute.value()) {
                if !matches!(attribute_name, "fill" | "stroke") {
                    return Err(unsafe_svg());
                }
                referenced.insert(identifier);
            } else if contains_unsafe_reference(attribute.value()) {
                return Err(unsafe_svg());
            }
        }
    }
    if referenced
        .iter()
        .any(|identifier| !declared.contains(identifier))
    {
        return Err(unsafe_svg());
    }
    Ok(())
}

fn pack_revision(
    manifest: &ProviderPackManifest,
    assets: &[ProviderAsset],
) -> OperationResult<String> {
    let manifest = serde_json::to_vec(manifest)
        .map_err(|_| invalid_pack("provider pack manifest cannot be serialized"))?;
    let mut digest = Sha256::new();
    digest.update(b"stack-provider-pack-v1\0");
    digest.update(manifest);
    let mut assets = assets.iter().collect::<Vec<_>>();
    assets.sort_by(|left, right| left.path.cmp(&right.path));
    for asset in assets {
        digest.update(b"asset\0");
        digest.update(asset.path.as_bytes());
        digest.update(b"\0");
        digest.update(asset.svg.as_bytes());
    }
    Ok(prefixed_digest(digest.finalize()))
}

fn sha256(bytes: &[u8]) -> String {
    prefixed_digest(Sha256::digest(bytes))
}

fn prefixed_digest(digest: impl AsRef<[u8]>) -> String {
    let mut output = String::from("sha256:");
    for byte in digest.as_ref() {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn parse_view_box(value: Option<&str>) -> Option<[i32; 4]> {
    let value = value?;
    let values = value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == 4 && values[2] > 0 && values[3] > 0)
        .then(|| [values[0], values[1], values[2], values[3]])
}

fn local_reference(value: &str) -> Option<&str> {
    value
        .strip_prefix("url(#")
        .and_then(|value| value.strip_suffix(')'))
        .filter(|value| !value.is_empty())
}

fn contains_unsafe_reference(value: &str) -> bool {
    let lowercase = value.to_ascii_lowercase();
    lowercase.contains("url(")
        || lowercase.contains("javascript:")
        || lowercase.contains("data:")
        || lowercase.contains("http://")
        || lowercase.contains("https://")
        || lowercase.contains("//")
}

fn valid_provider_id(value: &str) -> bool {
    (2..=32).contains(&value.len())
        && value.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                byte.is_ascii_lowercase()
            } else {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
            }
        })
}

fn valid_namespaced_icon_id(value: &str) -> bool {
    let Some((provider, slug)) = value.split_once(':') else {
        return false;
    };
    valid_provider_id(provider)
        && (1..=64).contains(&slug.len())
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_asset_path(value: &str) -> bool {
    value.starts_with("assets/")
        && value.ends_with(".svg")
        && !value.contains('\0')
        && !value.split('/').any(|component| component == "..")
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn invalid_pack(reason: &'static str) -> OperationalError {
    OperationalError::InvalidProviderPack { reason }
}

fn unsafe_svg() -> OperationalError {
    invalid_pack("provider pack asset contains unsafe or unsupported SVG")
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use serde::Deserialize;

    use super::*;
    use crate::{Engine, Severity};

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FixtureInput {
        manifest: ProviderPackManifest,
        assets: Vec<FixtureAsset>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FixtureAsset {
        path: String,
        svg: String,
    }

    fn fixture_input() -> Result<FixtureInput, Box<dyn Error>> {
        let mut inputs: Vec<FixtureInput> = serde_json::from_str(include_str!(
            "../../../tests/fixtures/provider-pack-input.json"
        ))?;
        inputs
            .pop()
            .ok_or_else(|| "missing provider fixture".into())
    }

    fn fixture_pack() -> Result<ProviderPack, Box<dyn Error>> {
        let input = fixture_input()?;
        Ok(ProviderPack::new(
            input.manifest,
            input
                .assets
                .into_iter()
                .map(|asset| ProviderAsset::new(asset.path, asset.svg))
                .collect(),
        )?)
    }

    fn pack_with_svg(svg: &str) -> Result<ProviderPack, Box<dyn Error>> {
        let mut input = fixture_input()?;
        let digest = sha256(svg.as_bytes());
        input.manifest.icons[0].asset.original_sha256 = digest.clone();
        input.manifest.icons[0].asset.processed_sha256 = digest;
        input.assets[0].svg = svg.to_owned();
        Ok(ProviderPack::new(
            input.manifest,
            input
                .assets
                .into_iter()
                .map(|asset| ProviderAsset::new(asset.path, asset.svg))
                .collect(),
        )?)
    }

    #[test]
    fn valid_pack_is_content_addressed_and_renders_without_changing_kind()
    -> Result<(), Box<dyn Error>> {
        let pack = fixture_pack()?;
        assert_eq!(pack.manifest().provider.id, "example");
        assert!(valid_sha256(pack.revision()));
        assert_eq!(pack.assets[0].path(), "assets/storage.svg");
        assert!(pack.assets[0].svg().contains("#4285f4"));

        let packs = [pack];
        let engine = Engine::with_provider_packs(&packs)?;
        let source = b"stack 1.0 diagram \"Provider\" { node item \"Example Storage\" { kind queue icon \"example:storage\" } }";
        let checked = engine.check(source)?;
        let first = engine.render(source)?;
        let second = engine.render(source)?;
        assert!(checked.diagnostics.is_empty());
        assert_eq!(first, second);
        let svg = first.svg.ok_or("missing rendered SVG")?;
        assert!(svg.contains("data-node-kind=\"queue\""));
        assert!(svg.contains("data-icon-id=\"example:storage\""));
        assert!(svg.contains("fill=\"#4285f4\""));
        assert!(svg.contains(packs[0].revision()));
        assert_eq!(first.provider_notices.len(), 1);
        assert_eq!(first.provider_notices[0].provider_id, "example");
        assert_eq!(first.provider_notices[0].icons[0].id, "example:storage");
        assert_eq!(
            first.provider_notices[0].icons[0].product_name,
            "Example Storage"
        );
        Ok(())
    }

    #[test]
    fn missing_pack_preserves_the_existing_warning_and_fallback() -> Result<(), Box<dyn Error>> {
        let source = b"stack 1.0 diagram \"Missing\" { node item \"Data\" { kind storage icon \"example:storage\" } }";
        let output = Engine::bundled().render(source)?;
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, "STK5001");
        assert_eq!(output.diagnostics[0].severity, Severity::Warning);
        assert!(output.provider_notices.is_empty());
        assert!(
            output
                .svg
                .ok_or("missing fallback SVG")?
                .contains("data-icon-id=\"kind-external\"")
        );
        Ok(())
    }

    #[test]
    fn manifest_boundary_rejects_invalid_rights_identity_and_records() -> Result<(), Box<dyn Error>>
    {
        let input = fixture_input()?;
        let assets = input
            .assets
            .iter()
            .map(|asset| ProviderAsset::new(&asset.path, &asset.svg))
            .collect::<Vec<_>>();

        for mutate in [
            |manifest: &mut ProviderPackManifest| manifest.schema_version = "2.0".to_owned(),
            |manifest: &mut ProviderPackManifest| manifest.rights.redistribution.cargo = true,
            |manifest: &mut ProviderPackManifest| manifest.rights.processing.server_upload = true,
        ] {
            let mut manifest = input.manifest.clone();
            mutate(&mut manifest);
            assert!(matches!(
                ProviderPack::new(manifest, assets.clone()),
                Err(OperationalError::InvalidProviderPack { .. })
            ));
        }

        for mutate in [
            |manifest: &mut ProviderPackManifest| manifest.provider.id = "X".to_owned(),
            |manifest: &mut ProviderPackManifest| manifest.source.archive_sha256 = "bad".to_owned(),
            |manifest: &mut ProviderPackManifest| manifest.notice.attribution.clear(),
        ] {
            let mut manifest = input.manifest.clone();
            mutate(&mut manifest);
            assert!(matches!(
                ProviderPack::new(manifest, assets.clone()),
                Err(OperationalError::InvalidProviderPack { .. })
            ));
        }

        for mutate in [
            |manifest: &mut ProviderPackManifest| manifest.icons[0].id = "other:storage".to_owned(),
            |manifest: &mut ProviderPackManifest| {
                manifest.icons[0].asset.path = "../icon.svg".to_owned()
            },
            |manifest: &mut ProviderPackManifest| manifest.icons[0].asset.view_box[2] = 0,
        ] {
            let mut manifest = input.manifest.clone();
            mutate(&mut manifest);
            assert!(matches!(
                ProviderPack::new(manifest, assets.clone()),
                Err(OperationalError::InvalidProviderPack { .. })
            ));
        }
        Ok(())
    }

    #[test]
    fn assets_must_be_exact_unique_small_and_hash_matched() -> Result<(), Box<dyn Error>> {
        let input = fixture_input()?;
        assert!(ProviderPack::new(input.manifest.clone(), Vec::new()).is_err());
        assert!(
            ProviderPack::new(
                input.manifest.clone(),
                vec![ProviderAsset::new("assets/other.svg", &input.assets[0].svg)],
            )
            .is_err()
        );
        assert!(
            ProviderPack::new(
                input.manifest.clone(),
                vec![ProviderAsset::new(
                    &input.assets[0].path,
                    format!("{} ", input.assets[0].svg),
                )],
            )
            .is_err()
        );
        assert!(
            ProviderPack::new(
                input.manifest,
                vec![ProviderAsset::new(
                    "assets/storage.svg",
                    "x".repeat(MAX_PROVIDER_ASSET_BYTES + 1),
                )],
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn total_asset_bytes_are_bounded() -> Result<(), Box<dyn Error>> {
        let mut input = fixture_input()?;
        let original = input.manifest.icons[0].clone();
        input.manifest.icons = (0..33)
            .map(|index| {
                let mut icon = original.clone();
                icon.id = format!("example:storage-{index}");
                icon.asset.path = format!("assets/storage-{index}.svg");
                icon
            })
            .collect();
        let assets = (0..33)
            .map(|index| {
                ProviderAsset::new(
                    format!("assets/storage-{index}.svg"),
                    "x".repeat(MAX_PROVIDER_ASSET_BYTES),
                )
            })
            .collect();
        assert!(matches!(
            ProviderPack::new(input.manifest, assets),
            Err(OperationalError::InvalidProviderPack {
                reason: "provider pack assets exceed the total size limit"
            })
        ));
        Ok(())
    }

    #[test]
    fn unsafe_or_unsupported_svg_is_rejected_after_hash_verification() {
        for svg in [
            "<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"/>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><script/></svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path onload=\"alert(1)\"/></svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path fill=\"https://example.com/icon.svg\"/></svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path fill=\"url(#missing)\"/></svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 20 20\"/>",
            "<svg viewBox=\"0 0 24 24\"/>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\">visible</svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><svg viewBox=\"0 0 1 1\"/></svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><defs><path/></defs></svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path id=\"shape\"/></svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><defs><linearGradient id=\"stack-paint\"/></defs><path d=\"url(#stack-paint)\"/></svg>",
        ] {
            assert!(
                pack_with_svg(svg).is_err(),
                "unsafe SVG was accepted: {svg}"
            );
        }
    }

    #[test]
    fn namespaced_local_gradients_are_accepted() {
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><defs><linearGradient id=\"stack-paint\"><stop offset=\"0\" stop-color=\"#000000\"/><stop offset=\"1\" stop-color=\"#ffffff\"/></linearGradient></defs><path fill=\"url(#stack-paint)\" d=\"M0 0h24v24H0z\"/></svg>";
        assert!(pack_with_svg(svg).is_ok());
    }

    #[test]
    fn duplicate_provider_namespaces_and_excessive_pack_counts_are_rejected()
    -> Result<(), Box<dyn Error>> {
        let pack = fixture_pack()?;
        assert!(matches!(
            Engine::with_provider_packs(&[pack.clone(), pack.clone()]),
            Err(OperationalError::InvalidProviderPack {
                reason: "provider namespaces must be unique"
            })
        ));
        let packs = vec![pack; 33];
        assert!(matches!(
            Engine::with_provider_packs(&packs),
            Err(OperationalError::InvalidProviderPack {
                reason: "an engine may contain at most 32 provider packs"
            })
        ));
        Ok(())
    }
}

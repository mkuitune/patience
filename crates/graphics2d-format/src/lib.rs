//! Portable Protobuf documents resolved into context-local graphics handles.

use graphics2d::{
    AssetId, Bounds, Document, Handedness, Instance, OutlineStyle, Paint, Placement, Resources,
    Shadow, SimplePlacement, StyleId, SvgAsset, Vec2, View, WidthUnit,
};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use uuid::Uuid;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/graphics.v1.rs"));
    include!(concat!(env!("OUT_DIR"), "/graphics.v1.serde.rs"));
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct LoadError(pub String);

fn fail(message: impl Into<String>) -> LoadError {
    LoadError(message.into())
}
fn uuid(text: &str) -> Result<Uuid, LoadError> {
    let value = Uuid::parse_str(text).map_err(|_| fail(format!("invalid UUID: {text}")))?;
    if value.is_nil() {
        return Err(fail("nil UUID is not a resource identity"));
    }
    Ok(value)
}
fn point(value: Option<proto::Point>) -> Result<Vec2, LoadError> {
    let value = value.ok_or_else(|| fail("missing point"))?;
    Ok(Vec2::new(value.x, value.y))
}
fn bounds(value: Option<proto::Bounds>) -> Result<Bounds, LoadError> {
    let value = value.ok_or_else(|| fail("missing bounds"))?;
    Bounds::new(point(value.min)?, point(value.max)?).map_err(|e| fail(e.to_string()))
}
fn color(value: Option<proto::Color>) -> Result<[u8; 4], LoadError> {
    let c = value.ok_or_else(|| fail("missing color"))?;
    let channel = |v| u8::try_from(v).map_err(|_| fail("color channel exceeds 255"));
    Ok([channel(c.r)?, channel(c.g)?, channel(c.b)?, channel(c.a)?])
}
fn json<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, LoadError> {
    serde_json::from_str(text).map_err(|e| fail(e.to_string()))
}

fn width_unit(value: i32) -> Result<WidthUnit, LoadError> {
    match proto::WidthUnit::try_from(value) {
        Ok(proto::WidthUnit::Source) => Ok(WidthUnit::Source),
        Ok(proto::WidthUnit::LogicalPixels) => Ok(WidthUnit::LogicalPixels),
        _ => Err(fail("unknown style width unit")),
    }
}

fn blur(value: f32) -> Result<f32, LoadError> {
    if !value.is_finite() || value < 0.0 {
        return Err(fail("blur must be finite and nonnegative"));
    }
    Ok(value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DocumentId(pub usize);

#[derive(Clone, Copy)]
enum ResourceId {
    Asset(AssetId),
    Style(StyleId),
    Document(DocumentId),
}

/// Handles belong to this immutable context and must not cross context boundaries.
pub struct Context {
    pub resources: Arc<Resources>,
    library_uuid: Uuid,
    ids: HashMap<Uuid, ResourceId>,
    asset_uuids: Vec<Uuid>,
    style_uuids: Vec<Uuid>,
    documents: Vec<(Uuid, Document, Bounds)>,
}

impl Context {
    /// Loads content through the caller's resolver; filesystem access is not required.
    pub fn load(
        manifest: &str,
        mut read: impl FnMut(&str) -> Result<String, LoadError>,
    ) -> Result<Self, LoadError> {
        let library: proto::Library = json(manifest)?;
        if library.version != 1 {
            return Err(fail("unsupported graphics library version"));
        }
        let library_uuid = uuid(&library.uuid)?;
        let mut ids = HashMap::new();
        let mut resources = Resources::default();
        let mut asset_uuids = Vec::new();
        let mut style_uuids = Vec::new();
        let mut documents = Vec::new();
        let mut paths = std::collections::HashSet::new();
        let mut stored_documents = Vec::new();
        for entry in library.resources {
            let id = uuid(&entry.uuid)?;
            if id == library_uuid || ids.contains_key(&id) {
                return Err(fail(format!("duplicate UUID: {id}")));
            }
            if entry.path.is_empty()
                || entry
                    .path
                    .split(['/', '\\'])
                    .any(|p| p.is_empty() || p == "." || p == "..")
                || entry.path.contains(':')
                || !paths.insert(entry.path.clone())
            {
                return Err(fail(format!(
                    "invalid or duplicate resource path: {}",
                    entry.path
                )));
            }
            let text = read(&entry.path).map_err(|e| fail(format!("{}: {e}", entry.path)))?;
            let resolved = match entry.source.ok_or_else(|| fail("missing resource kind"))? {
                proto::resource::Source::Svg(source) => {
                    let handle = AssetId(resources.assets.len());
                    resources.assets.push(SvgAsset {
                        name: entry.path,
                        svg: text,
                        bounds: bounds(source.bounds)?,
                    });
                    asset_uuids.push(id);
                    ResourceId::Asset(handle)
                }
                proto::resource::Source::Style(_) => {
                    let value: proto::Style = json(&text)?;
                    if uuid(&value.uuid)? != id {
                        return Err(fail("style UUID does not match manifest"));
                    }
                    if !value.outline_width.is_finite() || value.outline_width <= 0.0 {
                        return Err(fail("outline width must be finite and positive"));
                    }
                    let unit = width_unit(value.outline_unit)?;
                    let shadow = value
                        .shadow
                        .map(|s| {
                            let offset = point(s.offset)?;
                            if !offset.is_finite() {
                                return Err(fail("shadow offset must be finite"));
                            }
                            Ok(Shadow {
                                color: color(s.color)?,
                                offset,
                                blur: blur(s.blur)?,
                                unit: width_unit(s.unit)?,
                            })
                        })
                        .transpose()?;
                    let handle = StyleId(resources.styles.len());
                    resources.styles.push(OutlineStyle {
                        color: color(value.outline_color)?,
                        width: value.outline_width,
                        unit,
                        fill_color: value.fill_color.map(|c| color(Some(c))).transpose()?,
                        blur: blur(value.outline_blur)?,
                        shadow,
                    });
                    style_uuids.push(id);
                    ResourceId::Style(handle)
                }
                proto::resource::Source::Document(_) => {
                    let value: proto::Document = json(&text)?;
                    if uuid(&value.uuid)? != id {
                        return Err(fail("document UUID does not match manifest"));
                    }
                    let handle = DocumentId(stored_documents.len());
                    stored_documents.push(value);
                    ResourceId::Document(handle)
                }
            };
            ids.insert(id, resolved);
        }
        for value in stored_documents {
            let id = uuid(&value.uuid)?;
            let view = bounds(value.view)?;
            let mut document = Document::default();
            for instance in value.instances {
                let asset = match ids.get(&uuid(&instance.asset_uuid)?) {
                    Some(ResourceId::Asset(id)) => *id,
                    _ => {
                        return Err(fail(format!(
                            "unresolved SVG reference: {}",
                            instance.asset_uuid
                        )));
                    }
                };
                let style = instance
                    .style_uuid
                    .as_deref()
                    .map(|text| match ids.get(&uuid(text)?) {
                        Some(ResourceId::Style(id)) => Ok(*id),
                        _ => Err(fail(format!("unresolved style reference: {text}"))),
                    })
                    .transpose()?;
                let Some(proto::instance::Placement::Simple(p)) = instance.placement else {
                    return Err(fail("missing placement"));
                };
                let handedness = match p.handedness {
                    1 => Handedness::Positive,
                    -1 => Handedness::Negative,
                    _ => return Err(fail("handedness must be +1 or -1")),
                };
                let placement = SimplePlacement::new(
                    point(p.position)?,
                    point(p.x_direction)?,
                    p.scale,
                    handedness,
                )
                .map_err(|e| fail(e.to_string()))?;
                if resources.assets[asset.0]
                    .bounds
                    .corners()
                    .iter()
                    .any(|point| !placement.matrix().transform_point2(*point).is_finite())
                {
                    return Err(fail(format!(
                        "document {id}: placement exceeds finite coordinate range"
                    )));
                }
                let paint = if instance.color.is_some() {
                    Paint::Solid(color(instance.color)?)
                } else {
                    Paint::Original
                };
                document.instances.push(Instance {
                    asset,
                    placement: Placement::Simple(placement),
                    paint,
                    style,
                });
            }
            documents.push((id, document, view));
        }
        Ok(Self {
            resources: Arc::new(resources),
            library_uuid,
            ids,
            asset_uuids,
            style_uuids,
            documents,
        })
    }

    pub fn document_id(&self, text: &str) -> Result<DocumentId, LoadError> {
        match self.ids.get(&uuid(text)?) {
            Some(ResourceId::Document(id)) => Ok(*id),
            _ => Err(fail(format!("unresolved document reference: {text}"))),
        }
    }
    pub fn asset_id(&self, text: &str) -> Result<AssetId, LoadError> {
        match self.ids.get(&uuid(text)?) {
            Some(ResourceId::Asset(id)) => Ok(*id),
            _ => Err(fail(format!("unresolved SVG reference: {text}"))),
        }
    }
    pub fn document(&self, id: DocumentId) -> &Document {
        &self.documents[id.0].1
    }
    pub fn view(&self, id: DocumentId) -> View<'_> {
        View::new(self.document(id), self.documents[id.0].2)
    }
    pub fn library_uuid(&self) -> Uuid {
        self.library_uuid
    }
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Restores persistent identities; context-local integer handles never enter storage.
    pub fn export_document(&self, id: DocumentId) -> proto::Document {
        let (id, document, view) = &self.documents[id.0];
        proto::Document {
            uuid: id.to_string(),
            view: Some(stored_bounds(*view)),
            instances: document
                .instances
                .iter()
                .map(|i| {
                    let Placement::Simple(p) = i.placement;
                    proto::Instance {
                        asset_uuid: self.asset_uuids[i.asset.0].to_string(),
                        placement: Some(proto::instance::Placement::Simple(
                            proto::SimplePlacement {
                                position: Some(stored_point(p.position())),
                                x_direction: Some(stored_point(p.x_direction())),
                                scale: p.scale(),
                                handedness: i32::from(i8::from(p.handedness())),
                            },
                        )),
                        color: match i.paint {
                            Paint::Original => None,
                            Paint::Solid(c) => Some(stored_color(c)),
                        },
                        style_uuid: i.style.map(|id| self.style_uuids[id.0].to_string()),
                    }
                })
                .collect(),
        }
    }
}

fn stored_point(p: Vec2) -> proto::Point {
    proto::Point { x: p.x, y: p.y }
}
fn stored_bounds(b: Bounds) -> proto::Bounds {
    proto::Bounds {
        min: Some(stored_point(b.min())),
        max: Some(stored_point(b.max())),
    }
}
fn stored_color([r, g, b, a]: [u8; 4]) -> proto::Color {
    proto::Color {
        r: r.into(),
        g: g.into(),
        b: b.into(),
        a: a.into(),
    }
}

pub struct DeckBindings {
    pub faces: [DocumentId; 52],
    pub back: DocumentId,
    pub suits: [AssetId; 4],
    pub ranks: [AssetId; 13],
}

impl DeckBindings {
    pub fn load(text: &str, context: &Context) -> Result<Self, LoadError> {
        let deck: proto::Deck = json(text)?;
        if uuid(&deck.library_uuid)? != context.library_uuid {
            return Err(fail("deck references another library"));
        }
        let mut faces = [None; 52];
        for face in deck.faces {
            if !(1..=13).contains(&face.rank) || !(1..=4).contains(&face.suit) {
                return Err(fail("invalid card rank or suit"));
            }
            let index = (face.suit as usize - 1) * 13 + face.rank as usize - 1;
            if faces[index]
                .replace(context.document_id(&face.document_uuid)?)
                .is_some()
            {
                return Err(fail("duplicate card binding"));
            }
        }
        let faces = faces
            .into_iter()
            .map(|v| v.ok_or_else(|| fail("missing card binding")))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| fail("expected 52 faces"))?;
        let assets = |ids: Vec<String>| {
            ids.iter()
                .map(|s| context.asset_id(s))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(Self {
            faces,
            back: context.document_id(&deck.back_uuid)?,
            suits: assets(deck.suit_asset_uuids)?
                .try_into()
                .map_err(|_| fail("expected four suit assets"))?,
            ranks: assets(deck.rank_asset_uuids)?
                .try_into()
                .map_err(|_| fail("expected thirteen rank assets"))?,
        })
    }
}

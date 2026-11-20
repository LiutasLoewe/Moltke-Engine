//! Renderer-neutral snapshots and data-bound UI values.
//!
//! This crate keeps presentation data outside the authoritative simulation.
//! A frontend can consume the snapshots through `wgpu`, another renderer, or
//! a test/debug view without gaining mutable access to game state.

use moltke_core::{EdgeId, MapGraph, NodeId, Position3, Tick};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAX_TOOLTIP_DEPTH: usize = 32;
const MAX_TOOLTIP_NODES: usize = 1_024;
const MAX_UI_FIELDS: usize = 16_384;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MapModeId(String);

impl MapModeId {
    pub fn new(value: impl Into<String>) -> Result<Self, PresentationError> {
        let value = value.into();
        if valid_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(PresentationError::InvalidIdentifier(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeStyle {
    pub fill_rgba: [u8; 4],
    pub border_rgba: [u8; 4],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MapModeDefinition {
    pub id: MapModeId,
    /// First matching tag wins. An explicit order makes overlapping styles
    /// independent of hash-map iteration or content load order.
    pub tag_priority: Vec<String>,
    pub styles: BTreeMap<String, NodeStyle>,
    pub default_style: NodeStyle,
}

impl MapModeDefinition {
    pub fn validate(&self) -> Result<(), PresentationError> {
        if !valid_identifier(self.id.as_str()) {
            return Err(PresentationError::InvalidMapMode(self.id.clone()));
        }
        let mut seen = BTreeSet::new();
        for tag in &self.tag_priority {
            if !valid_identifier(tag) || !seen.insert(tag) {
                return Err(PresentationError::InvalidMapMode(self.id.clone()));
            }
            if !self.styles.contains_key(tag) {
                return Err(PresentationError::MissingTagStyle {
                    mode: self.id.clone(),
                    tag: tag.clone(),
                });
            }
        }
        if self.styles.keys().any(|tag| !valid_identifier(tag)) {
            return Err(PresentationError::InvalidMapMode(self.id.clone()));
        }
        Ok(())
    }

    fn style_for<'a>(&'a self, tags: &BTreeSet<String>) -> &'a NodeStyle {
        self.tag_priority
            .iter()
            .find_map(|tag| tags.contains(tag).then(|| self.styles.get(tag)).flatten())
            .unwrap_or(&self.default_style)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenderNode {
    pub id: NodeId,
    pub key: String,
    pub position: Option<Position3>,
    pub tags: BTreeSet<String>,
    pub style: NodeStyle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenderEdge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub base_cost: u32,
    pub tags: BTreeSet<String>,
}

/// Immutable frontend payload. Ordered graph storage gives a stable node and
/// edge order for renderer uploads and deterministic UI selection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MapRenderSnapshot {
    pub mode: MapModeId,
    pub tick: Tick,
    pub nodes: Vec<RenderNode>,
    pub edges: Vec<RenderEdge>,
}

impl MapRenderSnapshot {
    pub fn build(
        graph: &MapGraph,
        mode: &MapModeDefinition,
        tick: Tick,
    ) -> Result<Self, PresentationError> {
        mode.validate()?;
        let nodes = graph
            .nodes()
            .values()
            .map(|node| RenderNode {
                id: node.id,
                key: node.key.clone(),
                position: node.position,
                tags: node.tags.clone(),
                style: *mode.style_for(&node.tags),
            })
            .collect();
        let edges = graph
            .edges()
            .values()
            .map(|edge| RenderEdge {
                id: edge.id,
                from: edge.from,
                to: edge.to,
                base_cost: edge.base_cost,
                tags: edge.tags.clone(),
            })
            .collect();
        Ok(Self {
            mode: mode.id.clone(),
            tick,
            nodes,
            edges,
        })
    }
}

/// Values bound to a UI view without coupling the simulation to a widget
/// toolkit. Text can be literal or a localization key resolved by the client.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum UiValue {
    Text(String),
    LocalizationKey(String),
    Integer(i64),
    Fixed(moltke_core::Fixed),
    Boolean(bool),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiField {
    pub key: String,
    pub value: UiValue,
    #[serde(default)]
    pub tooltip: Option<TooltipNode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TooltipNode {
    pub title_key: String,
    #[serde(default)]
    pub value: Option<UiValue>,
    #[serde(default)]
    pub children: Vec<TooltipNode>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiDataView {
    pub fields: Vec<UiField>,
}

impl UiDataView {
    pub fn validate(&self) -> Result<(), PresentationError> {
        if self.fields.len() > MAX_UI_FIELDS {
            return Err(PresentationError::TooManyUiFields(self.fields.len()));
        }
        let mut keys = BTreeSet::new();
        let mut count = 0;
        for field in &self.fields {
            if !valid_identifier(&field.key) || !keys.insert(&field.key) {
                return Err(PresentationError::InvalidUiField(field.key.clone()));
            }
            if let Some(tooltip) = &field.tooltip {
                validate_tooltip(tooltip, 0, &mut count)?;
            }
        }
        Ok(())
    }
}

fn validate_tooltip(
    tooltip: &TooltipNode,
    depth: usize,
    count: &mut usize,
) -> Result<(), PresentationError> {
    if depth >= MAX_TOOLTIP_DEPTH {
        return Err(PresentationError::TooltipTooDeep);
    }
    *count += 1;
    if *count > MAX_TOOLTIP_NODES {
        return Err(PresentationError::TooManyTooltipNodes);
    }
    if !valid_identifier(&tooltip.title_key) {
        return Err(PresentationError::InvalidLocalizationKey(
            tooltip.title_key.clone(),
        ));
    }
    for child in &tooltip.children {
        validate_tooltip(child, depth + 1, count)?;
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentationError {
    InvalidIdentifier(String),
    InvalidMapMode(MapModeId),
    MissingTagStyle { mode: MapModeId, tag: String },
    InvalidUiField(String),
    InvalidLocalizationKey(String),
    TooltipTooDeep,
    TooManyTooltipNodes,
    TooManyUiFields(usize),
}

impl fmt::Display for PresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid presentation data: {self:?}")
    }
}

impl std::error::Error for PresentationError {}

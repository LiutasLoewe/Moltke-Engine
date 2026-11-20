//! Map import and generation helpers built on the genre-neutral core graph.

use image::ImageReader;
use moltke_core::{
    DeterministicRng, EdgeId, GraphError, MapEdge, MapGraph, MapNode, NodeId, Position3,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

const MAX_RASTER_PIXELS: usize = 100_000_000;
const MAX_GENERATED_NODES: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RgbRaster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Rgb>,
}

impl RgbRaster {
    pub fn validate(&self) -> Result<(), MapImportError> {
        let expected = pixel_count(self.width, self.height)?;
        if expected != self.pixels.len() {
            return Err(MapImportError::PixelCountMismatch {
                expected,
                actual: self.pixels.len(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionDefinition {
    pub node: MapNode,
    /// Cost to enter this region; used on directed boundary edges.
    pub movement_cost: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedMap {
    pub graph: MapGraph,
    pub average_height_by_node: BTreeMap<NodeId, i32>,
    /// Row-major raster lookup. Background pixels have no node.
    pub node_by_pixel: Vec<Option<NodeId>>,
}

/// Loads a PNG or BMP and converts it to an RGB province raster.
pub fn load_rgb_raster(path: impl AsRef<Path>) -> Result<RgbRaster, MapImportError> {
    let path = path.as_ref();
    let image = ImageReader::open(path)
        .map_err(|source| MapImportError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .decode()
        .map_err(|source| MapImportError::Image {
            path: path.to_path_buf(),
            source,
        })?
        .to_rgb8();
    let raster = RgbRaster {
        width: image.width(),
        height: image.height(),
        pixels: image
            .pixels()
            .map(|pixel| Rgb(pixel.0[0], pixel.0[1], pixel.0[2]))
            .collect(),
    };
    raster.validate()?;
    Ok(raster)
}

/// Loads a grayscale PNG or BMP heightmap as integer heights in 0..=255.
pub fn load_heightmap(path: impl AsRef<Path>) -> Result<(u32, u32, Vec<i32>), MapImportError> {
    let path = path.as_ref();
    let image = ImageReader::open(path)
        .map_err(|source| MapImportError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .decode()
        .map_err(|source| MapImportError::Image {
            path: path.to_path_buf(),
            source,
        })?
        .to_luma8();
    let width = image.width();
    let height = image.height();
    pixel_count(width, height)?;
    Ok((
        width,
        height,
        image.pixels().map(|pixel| i32::from(pixel.0[0])).collect(),
    ))
}

/// Converts an RGB region map into graph nodes and directed adjacency edges.
/// Colors and node IDs are resolved in sorted order, giving identical output
/// for identical raster, palette, heightmap, and first edge ID.
pub fn import_region_map(
    raster: &RgbRaster,
    palette: &BTreeMap<Rgb, RegionDefinition>,
    background: Option<Rgb>,
    heights: Option<&[i32]>,
    first_edge_id: u32,
) -> Result<ImportedMap, MapImportError> {
    raster.validate()?;
    if let Some(heights) = heights
        && heights.len() != raster.pixels.len()
    {
        return Err(MapImportError::HeightCountMismatch {
            expected: raster.pixels.len(),
            actual: heights.len(),
        });
    }

    let mut graph = MapGraph::default();
    let mut node_by_color = BTreeMap::new();
    let mut node_ids = BTreeSet::new();
    for (color, definition) in palette {
        if definition.movement_cost == 0 {
            return Err(MapImportError::ZeroMovementCost(definition.node.id));
        }
        if !node_ids.insert(definition.node.id) {
            return Err(MapImportError::DuplicateNodeId(definition.node.id));
        }
        let mut node = definition.node.clone();
        node.position = Some(Position3 { x: 0, y: 0, z: 0 });
        graph
            .add_node(node.clone())
            .map_err(MapImportError::Graph)?;
        node_by_color.insert(*color, node.id);
    }

    #[derive(Default)]
    struct RegionStats {
        count: u64,
        x_sum: i128,
        y_sum: i128,
        height_sum: i128,
    }

    let mut stats: BTreeMap<NodeId, RegionStats> = BTreeMap::new();
    let mut node_by_pixel = Vec::with_capacity(raster.pixels.len());
    for (index, color) in raster.pixels.iter().enumerate() {
        let node_id = if Some(*color) == background {
            None
        } else {
            Some(
                *node_by_color
                    .get(color)
                    .ok_or(MapImportError::UnknownColor(*color))?,
            )
        };
        if let Some(node_id) = node_id {
            let x = index as u64 % u64::from(raster.width);
            let y = index as u64 / u64::from(raster.width);
            let region = stats.entry(node_id).or_default();
            region.count += 1;
            region.x_sum += i128::from(x);
            region.y_sum += i128::from(y);
            if let Some(heights) = heights {
                region.height_sum += i128::from(heights[index]);
            }
        }
        node_by_pixel.push(node_id);
    }

    let mut average_height_by_node = BTreeMap::new();
    for (node_id, region) in &stats {
        let x = i64::try_from(region.x_sum / i128::from(region.count))
            .map_err(|_| MapImportError::CoordinateOverflow)?;
        let y = i64::try_from(region.y_sum / i128::from(region.count))
            .map_err(|_| MapImportError::CoordinateOverflow)?;
        let height = if heights.is_some() {
            i32::try_from(region.height_sum / i128::from(region.count))
                .map_err(|_| MapImportError::HeightOverflow)?
        } else {
            0
        };
        graph
            .set_node_position(
                *node_id,
                Some(Position3 {
                    x,
                    y,
                    z: i64::from(height),
                }),
            )
            .map_err(MapImportError::Graph)?;
        average_height_by_node.insert(*node_id, height);
    }
    for (color, definition) in palette {
        if !stats.contains_key(&definition.node.id) {
            return Err(MapImportError::RegionHasNoPixels(*color));
        }
    }

    let mut boundaries = BTreeSet::new();
    let width = raster.width as usize;
    let height = raster.height as usize;
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            let here = node_by_pixel[index];
            if x + 1 < width {
                add_boundary(here, node_by_pixel[index + 1], &mut boundaries);
            }
            if y + 1 < height {
                add_boundary(here, node_by_pixel[index + width], &mut boundaries);
            }
        }
    }

    let costs: BTreeMap<NodeId, u32> = palette
        .values()
        .map(|definition| (definition.node.id, definition.movement_cost))
        .collect();
    let mut next_edge_id = first_edge_id;
    for (left, right) in boundaries {
        let left_cost = *costs
            .get(&left)
            .ok_or(MapImportError::MissingMovementCost(left))?;
        let right_cost = *costs
            .get(&right)
            .ok_or(MapImportError::MissingMovementCost(right))?;
        graph
            .add_edge(MapEdge {
                id: EdgeId(next_edge_id),
                from: left,
                to: right,
                base_cost: right_cost,
                tags: BTreeSet::from(["raster_boundary".to_owned()]),
            })
            .map_err(MapImportError::Graph)?;
        next_edge_id = next_edge_id
            .checked_add(1)
            .ok_or(MapImportError::EdgeIdOverflow)?;
        graph
            .add_edge(MapEdge {
                id: EdgeId(next_edge_id),
                from: right,
                to: left,
                base_cost: left_cost,
                tags: BTreeSet::from(["raster_boundary".to_owned()]),
            })
            .map_err(MapImportError::Graph)?;
        next_edge_id = next_edge_id
            .checked_add(1)
            .ok_or(MapImportError::EdgeIdOverflow)?;
    }

    graph.validate().map_err(MapImportError::Graph)?;
    Ok(ImportedMap {
        graph,
        average_height_by_node,
        node_by_pixel,
    })
}

fn add_boundary(
    left: Option<NodeId>,
    right: Option<NodeId>,
    boundaries: &mut BTreeSet<(NodeId, NodeId)>,
) {
    let (Some(left), Some(right)) = (left, right) else {
        return;
    };
    if left != right {
        boundaries.insert(if left < right {
            (left, right)
        } else {
            (right, left)
        });
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridConfig {
    pub width: u32,
    pub height: u32,
    pub first_node_id: u32,
    pub first_edge_id: u32,
    pub wrap_x: bool,
    pub wrap_y: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedGrid {
    pub graph: MapGraph,
    pub terrain_by_node: BTreeMap<NodeId, u8>,
}

/// Generates a deterministic rectangular terrain graph. The seed affects
/// terrain tags and traversal costs; coordinates and IDs remain stable.
pub fn generate_grid(config: GridConfig, seed: u64) -> Result<GeneratedGrid, MapImportError> {
    let count = pixel_count(config.width, config.height)?;
    if count > MAX_GENERATED_NODES {
        return Err(MapImportError::TooManyGeneratedNodes(count));
    }
    let final_offset = u32::try_from(count - 1).map_err(|_| MapImportError::NodeIdOverflow)?;
    config
        .first_node_id
        .checked_add(final_offset)
        .ok_or(MapImportError::NodeIdOverflow)?;
    let mut rng = DeterministicRng::seeded(seed);
    let mut graph = MapGraph::default();
    let mut terrain_by_node = BTreeMap::new();
    let mut costs = BTreeMap::new();
    for y in 0..config.height {
        for x in 0..config.width {
            let offset = y * config.width + x;
            let id = NodeId(config.first_node_id + offset);
            let terrain = rng.range(5).ok_or(MapImportError::GeneratorFailure)? as u8;
            let cost = u32::from(terrain) + 1;
            graph
                .add_node(MapNode {
                    id,
                    key: format!("node_{x}_{y}"),
                    position: Some(Position3 {
                        x: i64::from(x),
                        y: i64::from(y),
                        z: i64::from(terrain),
                    }),
                    tags: BTreeSet::from(["generated".to_owned(), format!("terrain_{terrain}")]),
                })
                .map_err(MapImportError::Graph)?;
            terrain_by_node.insert(id, terrain);
            costs.insert(id, cost);
        }
    }

    let mut next_edge_id = config.first_edge_id;
    let mut add_pair = |from: NodeId, to: NodeId| -> Result<(), MapImportError> {
        for (source, target) in [(from, to), (to, from)] {
            let edge = MapEdge {
                id: EdgeId(next_edge_id),
                from: source,
                to: target,
                base_cost: *costs
                    .get(&target)
                    .ok_or(MapImportError::MissingMovementCost(target))?,
                tags: BTreeSet::from(["generated_route".to_owned()]),
            };
            graph.add_edge(edge).map_err(MapImportError::Graph)?;
            next_edge_id = next_edge_id
                .checked_add(1)
                .ok_or(MapImportError::EdgeIdOverflow)?;
        }
        Ok(())
    };
    for y in 0..config.height {
        for x in 0..config.width {
            let current = NodeId(config.first_node_id + y * config.width + x);
            if x + 1 < config.width {
                add_pair(
                    current,
                    NodeId(config.first_node_id + y * config.width + x + 1),
                )?;
            } else if config.wrap_x && config.width > 2 {
                add_pair(current, NodeId(config.first_node_id + y * config.width))?;
            }
            if y + 1 < config.height {
                add_pair(
                    current,
                    NodeId(config.first_node_id + (y + 1) * config.width + x),
                )?;
            } else if config.wrap_y && config.height > 2 {
                add_pair(current, NodeId(config.first_node_id + x))?;
            }
        }
    }
    drop(add_pair);
    graph.validate().map_err(MapImportError::Graph)?;
    Ok(GeneratedGrid {
        graph,
        terrain_by_node,
    })
}

fn pixel_count(width: u32, height: u32) -> Result<usize, MapImportError> {
    if width == 0 || height == 0 {
        return Err(MapImportError::EmptyRaster);
    }
    let count = usize::try_from(u64::from(width) * u64::from(height))
        .map_err(|_| MapImportError::RasterTooLarge)?;
    if count > MAX_RASTER_PIXELS {
        return Err(MapImportError::RasterTooLarge);
    }
    Ok(count)
}

#[derive(Debug)]
pub enum MapImportError {
    EmptyRaster,
    RasterTooLarge,
    PixelCountMismatch {
        expected: usize,
        actual: usize,
    },
    HeightCountMismatch {
        expected: usize,
        actual: usize,
    },
    UnknownColor(Rgb),
    DuplicateNodeId(NodeId),
    ZeroMovementCost(NodeId),
    RegionHasNoPixels(Rgb),
    MissingMovementCost(NodeId),
    CoordinateOverflow,
    HeightOverflow,
    EdgeIdOverflow,
    NodeIdOverflow,
    TooManyGeneratedNodes(usize),
    GeneratorFailure,
    Graph(GraphError),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Image {
        path: PathBuf,
        source: image::ImageError,
    },
}

impl fmt::Display for MapImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRaster => formatter.write_str("map raster dimensions must be nonzero"),
            Self::RasterTooLarge => {
                formatter.write_str("map raster exceeds the supported pixel limit")
            }
            Self::PixelCountMismatch { expected, actual } => {
                write!(formatter, "raster has {actual} pixels, expected {expected}")
            }
            Self::HeightCountMismatch { expected, actual } => write!(
                formatter,
                "heightmap has {actual} pixels, expected {expected}"
            ),
            Self::UnknownColor(color) => write!(
                formatter,
                "region map contains unmapped RGB color {color:?}"
            ),
            Self::DuplicateNodeId(id) => {
                write!(formatter, "multiple RGB colors map to node {}", id.0)
            }
            Self::ZeroMovementCost(id) => write!(formatter, "node {} has zero movement cost", id.0),
            Self::RegionHasNoPixels(color) => write!(
                formatter,
                "palette color {color:?} does not occur in the raster"
            ),
            Self::MissingMovementCost(id) => {
                write!(formatter, "node {} has no movement cost", id.0)
            }
            Self::CoordinateOverflow => {
                formatter.write_str("region centroid exceeds supported coordinates")
            }
            Self::HeightOverflow => {
                formatter.write_str("average region height exceeds supported values")
            }
            Self::EdgeIdOverflow => {
                formatter.write_str("edge ID range overflowed during map generation")
            }
            Self::NodeIdOverflow => {
                formatter.write_str("node ID range overflowed during map generation")
            }
            Self::TooManyGeneratedNodes(count) => write!(
                formatter,
                "grid has {count} nodes, exceeding the generation limit"
            ),
            Self::GeneratorFailure => {
                formatter.write_str("procedural generator failed to produce a value")
            }
            Self::Graph(error) => write!(formatter, "map graph is invalid: {error:?}"),
            Self::Io { path, source } => {
                write!(formatter, "could not read {}: {source}", path.display())
            }
            Self::Image { path, source } => {
                write!(formatter, "could not decode {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for MapImportError {}

use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u32);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdgeId(pub u32);

/// Coordinates are integer values in game-defined units, never simulation floats.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Position3 { pub x: i64, pub y: i64, pub z: i64 }

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MapNode {
    pub id: NodeId,
    pub key: String,
    pub position: Option<Position3>,
    pub tags: BTreeSet<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MapEdge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub base_cost: u32,
    pub tags: BTreeSet<String>,
}

/// Directed graph; represent a bidirectional route with two directed edges.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct MapGraph {
    nodes: BTreeMap<NodeId, MapNode>,
    edges: BTreeMap<EdgeId, MapEdge>,
    outgoing: BTreeMap<NodeId, BTreeSet<EdgeId>>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphError {
    DuplicateNode(NodeId), DuplicateNodeKey(String), DuplicateEdge(EdgeId),
    EmptyNodeKey, MissingEndpoint(NodeId), InvalidBaseCost(EdgeId),
    MissingNode(NodeId), CorruptAdjacency,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Path { pub nodes: Vec<NodeId>, pub edges: Vec<EdgeId>, pub total_cost: u64 }

impl MapGraph {
    pub fn nodes(&self) -> &BTreeMap<NodeId, MapNode> { &self.nodes }
    pub fn edges(&self) -> &BTreeMap<EdgeId, MapEdge> { &self.edges }

    pub fn add_node(&mut self, node: MapNode) -> Result<(), GraphError> {
        if node.key.trim().is_empty() { return Err(GraphError::EmptyNodeKey); }
        if self.nodes.contains_key(&node.id) { return Err(GraphError::DuplicateNode(node.id)); }
        if self.nodes.values().any(|n| n.key == node.key) {
            return Err(GraphError::DuplicateNodeKey(node.key));
        }
        self.outgoing.insert(node.id, BTreeSet::new());
        self.nodes.insert(node.id, node);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: MapEdge) -> Result<(), GraphError> {
        if self.edges.contains_key(&edge.id) { return Err(GraphError::DuplicateEdge(edge.id)); }
        if !self.nodes.contains_key(&edge.from) { return Err(GraphError::MissingEndpoint(edge.from)); }
        if !self.nodes.contains_key(&edge.to) { return Err(GraphError::MissingEndpoint(edge.to)); }
        if edge.base_cost == 0 { return Err(GraphError::InvalidBaseCost(edge.id)); }
        self.outgoing.get_mut(&edge.from).expect("validated endpoint").insert(edge.id);
        self.edges.insert(edge.id, edge);
        Ok(())
    }

    /// Overrides are precomputed by deterministic game logic. None blocks an edge.
    pub fn shortest_path(
        &self, start: NodeId, goal: NodeId,
        overrides: &BTreeMap<EdgeId, Option<u32>>,
    ) -> Result<Option<Path>, GraphError> {
        if !self.nodes.contains_key(&start) { return Err(GraphError::MissingNode(start)); }
        if !self.nodes.contains_key(&goal) { return Err(GraphError::MissingNode(goal)); }

        let mut distances = BTreeMap::from([(start, 0_u64)]);
        let mut previous: BTreeMap<NodeId, (NodeId, EdgeId)> = BTreeMap::new();
        let mut queue = BinaryHeap::from([Reverse((0_u64, start))]);
        while let Some(Reverse((cost, node))) = queue.pop() {
            if distances.get(&node) != Some(&cost) { continue; }
            if node == goal { break; }
            for edge_id in self.outgoing.get(&node).ok_or(GraphError::CorruptAdjacency)? {
                let edge = self.edges.get(edge_id).ok_or(GraphError::CorruptAdjacency)?;
                let weight = match overrides.get(edge_id) {
                    Some(None) => continue,
                    Some(Some(value)) => *value,
                    None => edge.base_cost,
                };
                let Some(next_cost) = cost.checked_add(u64::from(weight)) else { continue };
                if distances.get(&edge.to).is_none_or(|known| next_cost < *known) {
                    distances.insert(edge.to, next_cost);
                    previous.insert(edge.to, (node, *edge_id));
                    queue.push(Reverse((next_cost, edge.to)));
                }
            }
        }
        let Some(total_cost) = distances.get(&goal).copied() else { return Ok(None) };
        let mut nodes = vec![goal];
        let mut edges = Vec::new();
        let mut cursor = goal;
        while cursor != start {
            let (parent, edge) = previous.get(&cursor).copied().ok_or(GraphError::CorruptAdjacency)?;
            nodes.push(parent);
            edges.push(edge);
            cursor = parent;
        }
        nodes.reverse(); edges.reverse();
        Ok(Some(Path { nodes, edges, total_cost }))
    }

    pub fn validate(&self) -> Result<(), GraphError> {
        let mut expected: BTreeMap<NodeId, BTreeSet<EdgeId>> =
            self.nodes.keys().map(|id| (*id, BTreeSet::new())).collect();
        let mut keys = BTreeSet::new();
        for (id, node) in &self.nodes {
            if *id != node.id { return Err(GraphError::CorruptAdjacency); }
            if node.key.trim().is_empty() { return Err(GraphError::EmptyNodeKey); }
            if !keys.insert(&node.key) { return Err(GraphError::DuplicateNodeKey(node.key.clone())); }
        }
        for (id, edge) in &self.edges {
            if *id != edge.id { return Err(GraphError::CorruptAdjacency); }
            if edge.base_cost == 0 { return Err(GraphError::InvalidBaseCost(edge.id)); }
            let Some(outgoing) = expected.get_mut(&edge.from) else { return Err(GraphError::MissingEndpoint(edge.from)); };
            if !self.nodes.contains_key(&edge.to) { return Err(GraphError::MissingEndpoint(edge.to)); }
            outgoing.insert(edge.id);
        }
        if expected != self.outgoing { return Err(GraphError::CorruptAdjacency); }
        Ok(())
    }
}

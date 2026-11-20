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
pub struct Position3 {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

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
    #[serde(skip, default)]
    outgoing: BTreeMap<NodeId, BTreeSet<EdgeId>>,
    #[serde(skip, default)]
    node_keys: BTreeMap<String, NodeId>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphError {
    DuplicateNode(NodeId),
    DuplicateNodeKey(String),
    DuplicateEdge(EdgeId),
    EmptyNodeKey,
    MissingEndpoint(NodeId),
    InvalidBaseCost(EdgeId),
    MissingNode(NodeId),
    CorruptAdjacency,
    InvalidGoalHeuristic(NodeId),
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Path {
    pub nodes: Vec<NodeId>,
    pub edges: Vec<EdgeId>,
    pub total_cost: u64,
}

impl MapGraph {
    pub fn nodes(&self) -> &BTreeMap<NodeId, MapNode> {
        &self.nodes
    }
    pub fn edges(&self) -> &BTreeMap<EdgeId, MapEdge> {
        &self.edges
    }

    pub fn set_node_position(
        &mut self,
        id: NodeId,
        position: Option<Position3>,
    ) -> Result<(), GraphError> {
        let node = self.nodes.get_mut(&id).ok_or(GraphError::MissingNode(id))?;
        node.position = position;
        Ok(())
    }

    pub fn add_node(&mut self, node: MapNode) -> Result<(), GraphError> {
        if node.key.trim().is_empty() {
            return Err(GraphError::EmptyNodeKey);
        }
        if self.nodes.contains_key(&node.id) {
            return Err(GraphError::DuplicateNode(node.id));
        }
        if self.node_keys.contains_key(&node.key) {
            return Err(GraphError::DuplicateNodeKey(node.key));
        }
        self.node_keys.insert(node.key.clone(), node.id);
        self.outgoing.insert(node.id, BTreeSet::new());
        self.nodes.insert(node.id, node);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: MapEdge) -> Result<(), GraphError> {
        if self.edges.contains_key(&edge.id) {
            return Err(GraphError::DuplicateEdge(edge.id));
        }
        if !self.nodes.contains_key(&edge.from) {
            return Err(GraphError::MissingEndpoint(edge.from));
        }
        if !self.nodes.contains_key(&edge.to) {
            return Err(GraphError::MissingEndpoint(edge.to));
        }
        if edge.base_cost == 0 {
            return Err(GraphError::InvalidBaseCost(edge.id));
        }
        let Some(outgoing) = self.outgoing.get_mut(&edge.from) else {
            return Err(GraphError::CorruptAdjacency);
        };
        outgoing.insert(edge.id);
        self.edges.insert(edge.id, edge);
        Ok(())
    }

    /// Overrides are precomputed by deterministic game logic. None blocks an edge.
    pub fn shortest_path(
        &self,
        start: NodeId,
        goal: NodeId,
        overrides: &BTreeMap<EdgeId, Option<u32>>,
    ) -> Result<Option<Path>, GraphError> {
        self.a_star_path(start, goal, overrides, &BTreeMap::new())
    }

    /// Finds a path with deterministic A* ordering. The heuristic table must
    /// contain admissible lower bounds for this graph and use zero for `goal`;
    /// omitted nodes use a zero heuristic. A zero-valued table is equivalent
    /// to Dijkstra. Dynamic costs must be precomputed in `overrides` before the
    /// search begins, so the pathfinder never calls scripts.
    pub fn a_star_path(
        &self,
        start: NodeId,
        goal: NodeId,
        overrides: &BTreeMap<EdgeId, Option<u32>>,
        heuristic: &BTreeMap<NodeId, u64>,
    ) -> Result<Option<Path>, GraphError> {
        if !self.nodes.contains_key(&start) {
            return Err(GraphError::MissingNode(start));
        }
        if !self.nodes.contains_key(&goal) {
            return Err(GraphError::MissingNode(goal));
        }
        if heuristic.get(&goal).copied().unwrap_or(0) != 0 {
            return Err(GraphError::InvalidGoalHeuristic(goal));
        }

        let mut distances = BTreeMap::from([(start, 0_u64)]);
        let mut previous: BTreeMap<NodeId, (NodeId, EdgeId)> = BTreeMap::new();
        let start_priority = u128::from(heuristic.get(&start).copied().unwrap_or(0));
        let mut queue = BinaryHeap::from([Reverse((start_priority, 0_u64, start))]);
        while let Some(Reverse((_priority, cost, node))) = queue.pop() {
            if distances.get(&node) != Some(&cost) {
                continue;
            }
            if node == goal {
                break;
            }
            for edge_id in self
                .outgoing
                .get(&node)
                .ok_or(GraphError::CorruptAdjacency)?
            {
                let edge = self
                    .edges
                    .get(edge_id)
                    .ok_or(GraphError::CorruptAdjacency)?;
                let weight = match overrides.get(edge_id) {
                    Some(None) => continue,
                    Some(Some(value)) => *value,
                    None => edge.base_cost,
                };
                let Some(next_cost) = cost.checked_add(u64::from(weight)) else {
                    continue;
                };
                if distances
                    .get(&edge.to)
                    .is_none_or(|known| next_cost < *known)
                {
                    distances.insert(edge.to, next_cost);
                    previous.insert(edge.to, (node, *edge_id));
                    let estimate = u128::from(heuristic.get(&edge.to).copied().unwrap_or(0));
                    queue.push(Reverse((
                        u128::from(next_cost) + estimate,
                        next_cost,
                        edge.to,
                    )));
                }
            }
        }
        let Some(total_cost) = distances.get(&goal).copied() else {
            return Ok(None);
        };
        let mut nodes = vec![goal];
        let mut edges = Vec::new();
        let mut cursor = goal;
        while cursor != start {
            let (parent, edge) = previous
                .get(&cursor)
                .copied()
                .ok_or(GraphError::CorruptAdjacency)?;
            nodes.push(parent);
            edges.push(edge);
            cursor = parent;
        }
        nodes.reverse();
        edges.reverse();
        Ok(Some(Path {
            nodes,
            edges,
            total_cost,
        }))
    }

    /// Recreates adjacency and key indexes after deserializing graph data.
    pub fn rebuild_indexes(&mut self) -> Result<(), GraphError> {
        let mut outgoing: BTreeMap<NodeId, BTreeSet<EdgeId>> =
            self.nodes.keys().map(|id| (*id, BTreeSet::new())).collect();
        let mut node_keys = BTreeMap::new();
        for (id, node) in &self.nodes {
            if *id != node.id {
                return Err(GraphError::CorruptAdjacency);
            }
            if node.key.trim().is_empty() {
                return Err(GraphError::EmptyNodeKey);
            }
            if node_keys.insert(node.key.clone(), *id).is_some() {
                return Err(GraphError::DuplicateNodeKey(node.key.clone()));
            }
        }
        for (id, edge) in &self.edges {
            if *id != edge.id {
                return Err(GraphError::CorruptAdjacency);
            }
            if edge.base_cost == 0 {
                return Err(GraphError::InvalidBaseCost(edge.id));
            }
            let Some(edges) = outgoing.get_mut(&edge.from) else {
                return Err(GraphError::MissingEndpoint(edge.from));
            };
            if !self.nodes.contains_key(&edge.to) {
                return Err(GraphError::MissingEndpoint(edge.to));
            }
            edges.insert(*id);
        }
        self.outgoing = outgoing;
        self.node_keys = node_keys;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), GraphError> {
        let mut expected: BTreeMap<NodeId, BTreeSet<EdgeId>> =
            self.nodes.keys().map(|id| (*id, BTreeSet::new())).collect();
        let mut keys = BTreeMap::new();
        for (id, node) in &self.nodes {
            if *id != node.id {
                return Err(GraphError::CorruptAdjacency);
            }
            if node.key.trim().is_empty() {
                return Err(GraphError::EmptyNodeKey);
            }
            if keys.insert(node.key.clone(), *id).is_some() {
                return Err(GraphError::DuplicateNodeKey(node.key.clone()));
            }
        }
        for (id, edge) in &self.edges {
            if *id != edge.id {
                return Err(GraphError::CorruptAdjacency);
            }
            if edge.base_cost == 0 {
                return Err(GraphError::InvalidBaseCost(edge.id));
            }
            let Some(outgoing) = expected.get_mut(&edge.from) else {
                return Err(GraphError::MissingEndpoint(edge.from));
            };
            if !self.nodes.contains_key(&edge.to) {
                return Err(GraphError::MissingEndpoint(edge.to));
            }
            outgoing.insert(edge.id);
        }
        if expected != self.outgoing || keys != self.node_keys {
            return Err(GraphError::CorruptAdjacency);
        }
        Ok(())
    }
}

use glam::Vec3;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MorphologyNode {
    pub id: u32,
    pub length: f32,
    pub radius: f32,
    pub mass: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MorphologyEdge {
    pub source_node: u32,
    pub target_node: u32,
    pub joint_anchor: Vec3,
    pub joint_axis: Vec3,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MorphologyGenotype {
    pub nodes: Vec<MorphologyNode>,
    pub edges: Vec<MorphologyEdge>,
}

impl MorphologyGenotype {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: MorphologyNode) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: MorphologyEdge) {
        self.edges.push(edge);
    }
}


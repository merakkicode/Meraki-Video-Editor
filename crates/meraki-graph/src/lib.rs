#![forbid(unsafe_code)]
mod pipeline;
use meraki_core::{Error, GraphId, NodeId, Result, Time};
pub use pipeline::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Image,
    Mask,
    Scalar,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub enum Value {
    Scalar(f32),
    Vec2([f32; 2]),
    Color([f32; 4]),
    Bool(bool),
}
impl Value {
    fn finite(&self) -> bool {
        match self {
            Self::Scalar(v) => v.is_finite(),
            Self::Vec2(v) => v.iter().all(|x| x.is_finite()),
            Self::Color(v) => v.iter().all(|x| x.is_finite()),
            Self::Bool(_) => true,
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    Hold,
    Linear,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Keyframe {
    pub at: Time,
    pub value: Value,
    pub interpolation: Interpolation,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Parameter {
    pub value: Value,
    pub keys: Vec<Keyframe>,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Node {
    #[serde(default)]
    pub bypass: bool,
    pub id: NodeId,
    /// Versioned registry key, e.g. meraki.color.primary@1. Registry checks belong to compilation.
    pub effect: String,
    /// All declared inputs are required; optional sockets belong in a later schema version.
    pub inputs: Vec<Signal>,
    pub outputs: Vec<Signal>,
    pub parameters: BTreeMap<String, Parameter>,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port {
    pub node: NodeId,
    pub socket: usize,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: Port,
    pub to: Port,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct EffectGraph {
    pub id: GraphId,
    pub nodes: BTreeMap<NodeId, Node>,
    pub edges: Vec<Edge>,
    /// Output is a node's output socket. No special sink node required.
    pub output: Port,
}
impl EffectGraph {
    /// Structural validation only. The renderer separately validates registry schemas,
    /// parameter units/ranges, resource limits and backend support before compilation.
    pub fn validate(&self) -> Result<Vec<NodeId>> {
        let output = self.nodes.get(&self.output.node).ok_or(Error::NotFound)?;
        if output.outputs.get(self.output.socket) != Some(&Signal::Image) {
            return Err(Error::Invalid("graph output"));
        }
        let mut degrees: BTreeMap<NodeId, usize> = self.nodes.keys().map(|id| (*id, 0)).collect();
        let mut adjacency: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        let mut occupied = BTreeSet::new();
        for (id, node) in &self.nodes {
            if *id != node.id || node.effect.is_empty() {
                return Err(Error::Invalid("node identity"));
            }
            for p in node.parameters.values() {
                if !p.value.finite() {
                    return Err(Error::Invalid("parameter"));
                }
                for key in &p.keys {
                    if key.at < Time::ZERO
                        || !key.value.finite()
                        || std::mem::discriminant(&key.value) != std::mem::discriminant(&p.value)
                    {
                        return Err(Error::Invalid("keyframe"));
                    }
                    if matches!(&key.value, Value::Bool(_))
                        && key.interpolation != Interpolation::Hold
                    {
                        return Err(Error::Invalid("boolean interpolation"));
                    }
                }
                if p.keys.windows(2).any(|w| w[0].at >= w[1].at) {
                    return Err(Error::Invalid("keyframe ordering"));
                }
            }
        }
        for edge in &self.edges {
            let from = self.nodes.get(&edge.from.node).ok_or(Error::NotFound)?;
            let to = self.nodes.get(&edge.to.node).ok_or(Error::NotFound)?;
            let signal = from
                .outputs
                .get(edge.from.socket)
                .ok_or(Error::Invalid("output socket"))?;
            if to.inputs.get(edge.to.socket) != Some(signal) {
                return Err(Error::Invalid("socket type"));
            }
            if !occupied.insert((edge.to.node, edge.to.socket)) {
                return Err(Error::Invalid("multiple input writers"));
            }
            *degrees.get_mut(&edge.to.node).ok_or(Error::NotFound)? += 1;
            adjacency
                .entry(edge.from.node)
                .or_default()
                .push(edge.to.node);
        }
        for node in self.nodes.values() {
            for socket in 0..node.inputs.len() {
                if !occupied.contains(&(node.id, socket)) {
                    return Err(Error::Invalid("unconnected input"));
                }
            }
        }
        let mut ready: VecDeque<_> = degrees
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(id) = ready.pop_front() {
            order.push(id);
            if let Some(children) = adjacency.get(&id) {
                for child in children {
                    let degree = degrees.get_mut(child).ok_or(Error::NotFound)?;
                    *degree -= 1;
                    if *degree == 0 {
                        ready.push_back(*child);
                    }
                }
            }
        }
        if order.len() != self.nodes.len() {
            return Err(Error::Invalid("cycle"));
        }
        Ok(order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn graph() -> EffectGraph {
        let source = Node {
            bypass: false,
            id: NodeId(1),
            effect: "source@1".into(),
            inputs: vec![],
            outputs: vec![Signal::Image],
            parameters: BTreeMap::new(),
        };
        let fx = Node {
            bypass: false,
            id: NodeId(2),
            effect: "color@1".into(),
            inputs: vec![Signal::Image],
            outputs: vec![Signal::Image],
            parameters: BTreeMap::new(),
        };
        EffectGraph {
            id: GraphId(1),
            nodes: [(source.id, source), (fx.id, fx)].into(),
            edges: vec![Edge {
                from: Port {
                    node: NodeId(1),
                    socket: 0,
                },
                to: Port {
                    node: NodeId(2),
                    socket: 0,
                },
            }],
            output: Port {
                node: NodeId(2),
                socket: 0,
            },
        }
    }
    #[test]
    fn rejects_type_mismatch_and_duplicate_input() {
        let mut g = graph();
        assert_eq!(g.validate().unwrap(), vec![NodeId(1), NodeId(2)]);
        g.edges.push(g.edges[0].clone());
        assert!(g.validate().is_err());
        g.edges.pop();
        g.nodes.get_mut(&NodeId(2)).unwrap().inputs[0] = Signal::Mask;
        assert!(g.validate().is_err());
    }
    #[test]
    fn rejects_cycles() {
        let mut g = graph();
        g.nodes
            .get_mut(&NodeId(1))
            .unwrap()
            .inputs
            .push(Signal::Image);
        g.edges.push(Edge {
            from: g.output,
            to: Port {
                node: NodeId(1),
                socket: 0,
            },
        });
        assert_eq!(g.validate(), Err(Error::Invalid("cycle")));
    }
}

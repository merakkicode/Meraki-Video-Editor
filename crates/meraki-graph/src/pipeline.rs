use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectSettings {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub radius: f32,
    pub color_bypass: bool,
    pub blur_bypass: bool,
    pub color_connected: bool,
    pub blur_connected: bool,
}
impl Default for EffectSettings {
    fn default() -> Self {
        Self {
            brightness: 0.,
            contrast: 1.,
            saturation: 1.,
            radius: 0.,
            color_bypass: false,
            blur_bypass: false,
            color_connected: true,
            blur_connected: true,
        }
    }
}
impl EffectSettings {
    pub fn validate(&self) -> Result<()> {
        for (v, lo, hi) in [
            (self.brightness, -1., 1.),
            (self.contrast, 0., 2.),
            (self.saturation, 0., 2.),
            (self.radius, 0., 50.),
        ] {
            if !v.is_finite() || !(lo..=hi).contains(&v) {
                return Err(Error::Invalid("effect parameter range"));
            }
        }
        Ok(())
    }
    /// Disconnect removes a processor from the route and reconnects its neighbours.
    pub fn graph(self, id: GraphId) -> Result<EffectGraph> {
        self.validate()?;
        let scalar = |v| Parameter {
            value: Value::Scalar(v),
            keys: vec![],
        };
        let mut nodes = BTreeMap::new();
        for (n, name, parameters, bypass) in [
            (1, "source@1", BTreeMap::new(), false),
            (
                2,
                "color@1",
                BTreeMap::from([
                    ("brightness".into(), scalar(self.brightness)),
                    ("contrast".into(), scalar(self.contrast)),
                    ("saturation".into(), scalar(self.saturation)),
                ]),
                self.color_bypass,
            ),
            (
                3,
                "blur@1",
                BTreeMap::from([("radius".into(), scalar(self.radius))]),
                self.blur_bypass,
            ),
            (4, "output@1", BTreeMap::new(), false),
        ] {
            nodes.insert(
                NodeId(n),
                Node {
                    id: NodeId(n),
                    effect: name.into(),
                    inputs: if n == 1 { vec![] } else { vec![Signal::Image] },
                    outputs: vec![Signal::Image],
                    parameters,
                    bypass,
                },
            );
        }
        // Disconnected nodes keep an input from Media, but have no path to Output.
        let mut previous = 1;
        let mut edges = vec![];
        for (n, connected) in [
            (2, self.color_connected),
            (3, self.blur_connected),
            (4, true),
        ] {
            edges.push(Edge {
                from: Port {
                    node: NodeId(if connected { previous } else { 1 }),
                    socket: 0,
                },
                to: Port {
                    node: NodeId(n),
                    socket: 0,
                },
            });
            if connected {
                previous = n;
            }
        }
        let graph = EffectGraph {
            id,
            nodes,
            edges,
            output: Port {
                node: NodeId(4),
                socket: 0,
            },
        };
        compile(&graph)?;
        Ok(graph)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operation {
    Color {
        brightness: f32,
        contrast: f32,
        saturation: f32,
    },
    Blur {
        radius: f32,
    },
}

/// Compiles the reachable unary image path; rejects unsupported nodes/ports/animation.
/// Independent branches are validated but not executed when unreachable from Output.
pub fn compile(graph: &EffectGraph) -> Result<Vec<Operation>> {
    if graph.nodes.len() > 32 || graph.edges.len() > 32 {
        return Err(Error::Invalid("graph resource limit"));
    }
    graph.validate()?;
    for node in graph.nodes.values() {
        let names: &[&str] = match node.effect.as_str() {
            "source@1" | "output@1" => &[],
            "color@1" => &["brightness", "contrast", "saturation"],
            "blur@1" => &["radius"],
            _ => return Err(Error::Invalid("unsupported effect")),
        };
        if node.outputs != [Signal::Image]
            || (node.effect == "source@1" && !node.inputs.is_empty())
            || (node.effect != "source@1" && node.inputs != [Signal::Image])
        {
            return Err(Error::Invalid("effect socket schema"));
        }
        for (key, p) in &node.parameters {
            if !names.contains(&key.as_str()) || !p.keys.is_empty() {
                return Err(Error::Invalid("unsupported effect parameter or animation"));
            }
        }
        let settings = settings(node)?;
        settings.validate()?;
    }
    let mut current = graph.output.node;
    let mut reversed = vec![];
    loop {
        let node = &graph.nodes[&current];
        let s = settings(node)?;
        if !node.bypass {
            match node.effect.as_str() {
                "color@1" => reversed.push(Operation::Color {
                    brightness: s.brightness,
                    contrast: s.contrast,
                    saturation: s.saturation,
                }),
                "blur@1" => reversed.push(Operation::Blur { radius: s.radius }),
                _ => {}
            }
        }
        if node.effect == "source@1" {
            break;
        }
        current = graph
            .edges
            .iter()
            .find(|e| e.to.node == current)
            .ok_or(Error::Invalid("missing image input"))?
            .from
            .node;
    }
    reversed.reverse();
    Ok(reversed)
}
fn settings(node: &Node) -> Result<EffectSettings> {
    let mut s = EffectSettings::default();
    for (name, parameter) in &node.parameters {
        let Value::Scalar(value) = parameter.value else {
            return Err(Error::Invalid("scalar effect parameter required"));
        };
        match name.as_str() {
            "brightness" => s.brightness = value,
            "contrast" => s.contrast = value,
            "saturation" => s.saturation = value,
            "radius" => s.radius = value,
            _ => {}
        }
    }
    Ok(s)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_bypass_and_validation() {
        let mut s = EffectSettings {
            radius: 12.,
            ..Default::default()
        };
        assert_eq!(compile(&s.graph(GraphId(1)).unwrap()).unwrap().len(), 2);
        s.color_connected = false;
        assert_eq!(
            compile(&s.graph(GraphId(1)).unwrap()).unwrap(),
            vec![Operation::Blur { radius: 12. }]
        );
        s.blur_bypass = true;
        assert!(compile(&s.graph(GraphId(1)).unwrap()).unwrap().is_empty());
        s.radius = f32::NAN;
        assert!(s.graph(GraphId(1)).is_err());
        let mut g = EffectSettings::default().graph(GraphId(1)).unwrap();
        g.nodes.get_mut(&NodeId(3)).unwrap().effect = "unknown@1".into();
        assert!(compile(&g).is_err());
    }
}

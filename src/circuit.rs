use std::collections::HashMap;

use crate::config::SimConfig;
use crate::device::Device;
use crate::node::Node;
use crate::state::StateVectors;

/// Circuit topology — the immutable structure of a SPICE circuit.
///
/// Holds nodes, devices, and the node name → equation number mapping.
/// Matches the topology portion of ngspice's CKTcircuit.
pub struct Circuit {
    /// All nodes, indexed by equation number. Index 0 = ground.
    pub nodes: Vec<Node>,
    /// Node name → equation number lookup.
    node_map: HashMap<String, usize>,
    /// All device instances.
    pub devices: Vec<Box<dyn Device>>,
    /// Device state vectors.
    pub states: StateVectors,
}

impl Circuit {
    pub fn new() -> Self {
        let ground = Node::voltage("0");
        Self {
            nodes: vec![ground],
            node_map: HashMap::from([("0".to_string(), 0)]),
            devices: Vec::new(),
            states: StateVectors::new(),
        }
    }

    /// Get or create a voltage node, returning its equation number.
    /// Matches ngspice CKTmkVolt + CKTlinkEq: monotonic assignment.
    pub fn node(&mut self, name: &str) -> usize {
        if let Some(&eq) = self.node_map.get(name) {
            return eq;
        }
        let eq = self.nodes.len();
        self.nodes.push(Node::voltage(name));
        self.node_map.insert(name.to_string(), eq);
        eq
    }

    /// Create a branch (current) equation, returning its equation number.
    /// Matches ngspice CKTmkCur.
    pub fn branch(&mut self, name: &str) -> usize {
        let eq = self.nodes.len();
        self.nodes.push(Node::current(name));
        self.node_map.insert(name.to_string(), eq);
        eq
    }

    /// Return equation map: Vec of (eq_number, name, type) for all nodes/branches.
    pub fn equation_map(&self) -> Vec<(usize, String, &'static str)> {
        self.nodes.iter().enumerate().map(|(i, n)| {
            let kind = if n.node_type == crate::node::NodeType::Voltage { "V" } else { "I" };
            (i, n.name.clone(), kind)
        }).collect()
    }

    /// Add a device to the circuit.
    pub fn add_device(&mut self, device: Box<dyn Device>) {
        self.devices.push(device);
    }

    /// Number of equations (CKTmaxEqNum), including ground.
    pub fn num_equations(&self) -> usize {
        self.nodes.len()
    }

    /// Look up equation number by node name.
    pub fn find_node(&self, name: &str) -> Option<usize> {
        self.node_map.get(name).copied()
    }

    /// Run device setup: allocate state vectors, then finalize.
    /// Matches ngspice CKTsetup DEVsetup loop + state array allocation.
    pub fn setup(&mut self) {
        for device in &mut self.devices {
            device.setup(&mut self.states);
        }
        self.states.finalize();
    }

    /// Run temperature preprocessing on all devices.
    pub fn temperature(&mut self, config: &SimConfig) {
        for device in &mut self.devices {
            device.temperature(config.temp, config.tnom);
        }
    }
}

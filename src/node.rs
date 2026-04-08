/// Node type — matches ngspice SP_VOLTAGE / SP_CURRENT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// Voltage node (SP_VOLTAGE = 3 in ngspice).
    Voltage,
    /// Current branch equation (SP_CURRENT = 4 in ngspice).
    Current,
}

/// A circuit node or branch equation.
#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    pub node_type: NodeType,
    /// Initial condition (.IC value), if specified.
    pub ic: Option<f64>,
    /// Nodeset value (.NODESET), if specified.
    pub nodeset: Option<f64>,
}

impl Node {
    pub fn voltage(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            node_type: NodeType::Voltage,
            ic: None,
            nodeset: None,
        }
    }

    pub fn current(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            node_type: NodeType::Current,
            ic: None,
            nodeset: None,
        }
    }
}

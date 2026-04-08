//! Minimal SPICE netlist parser — extracts just enough for layout.

/// A parsed component from a SPICE netlist.
#[derive(Debug, Clone)]
pub struct Component {
    /// Reference designator: "R1", "M1", "V1"
    pub ref_des: String,
    /// Type prefix: 'R', 'C', 'L', 'V', 'I', 'D', 'M', 'Q', 'J'
    pub comp_type: char,
    /// Node names this component connects to, in SPICE order.
    /// Two-terminal: [pos, neg] or [node1, node2]
    /// MOSFET: [drain, gate, source, bulk]
    /// BJT: [collector, base, emitter]
    pub nodes: Vec<String>,
    /// Component value string: "1k", "100n", "DC 5"
    pub value: String,
    /// Model name (if any): "NMOS", "2N2222"
    pub model: Option<String>,
}

/// A parsed netlist ready for layout.
#[derive(Debug, Clone)]
pub struct Netlist {
    pub components: Vec<Component>,
    /// All unique node names (excluding "0" ground).
    pub nodes: Vec<String>,
    /// Title line (if any).
    pub title: Option<String>,
}

impl Netlist {
    /// Get all components connected to a given node.
    pub fn components_on_node(&self, node: &str) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| c.nodes.iter().any(|n| n == node))
            .collect()
    }

    /// Get all nodes in the circuit including ground "0".
    pub fn all_nodes(&self) -> Vec<&str> {
        let mut nodes: Vec<&str> = vec!["0"];
        for n in &self.nodes {
            nodes.push(n.as_str());
        }
        nodes
    }
}

/// Parse a SPICE netlist string into components and nodes.
pub fn parse(input: &str) -> Netlist {
    let mut components = Vec::new();
    let mut node_set = std::collections::HashSet::new();
    let mut title = None;
    let mut first_line = true;

    for raw_line in input.lines() {
        let line = raw_line.trim();

        // First non-empty line is the title
        if first_line && !line.is_empty() && !line.starts_with('*') && !line.starts_with('.') {
            // Could be title or could be a component — check if it starts with a device letter
            let first_char = line.chars().next().unwrap_or(' ');
            if !is_device_prefix(first_char) {
                title = Some(line.to_string());
                first_line = false;
                continue;
            }
        }
        first_line = false;

        // Skip empty lines, comments, and directives
        if line.is_empty() || line.starts_with('*') || line.starts_with('.') {
            continue;
        }

        let first_char = line.chars().next().unwrap_or(' ');
        if !is_device_prefix(first_char) {
            continue;
        }

        if let Some(comp) = parse_component(line) {
            for node in &comp.nodes {
                if node != "0" {
                    node_set.insert(node.clone());
                }
            }
            components.push(comp);
        }
    }

    let mut nodes: Vec<String> = node_set.into_iter().collect();
    nodes.sort();

    Netlist { components, nodes, title }
}

fn is_device_prefix(c: char) -> bool {
    matches!(
        c.to_ascii_uppercase(),
        'R' | 'C' | 'L' | 'V' | 'I' | 'D' | 'M' | 'Q' | 'J' | 'K' | 'T' | 'E' | 'F' | 'G' | 'H' | 'X'
    )
}

fn parse_component(line: &str) -> Option<Component> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let ref_des = tokens[0].to_string();
    let comp_type = ref_des.chars().next()?.to_ascii_uppercase();

    let (nodes, value, model) = match comp_type {
        // Two-terminal: R1 n1 n2 value
        'R' | 'C' | 'L' => {
            if tokens.len() < 4 {
                return None;
            }
            let nodes = vec![tokens[1].to_string(), tokens[2].to_string()];
            let value = tokens[3..].join(" ");
            (nodes, value, None)
        }
        // Voltage/current source: V1 n+ n- DC 5 or V1 n+ n- value
        'V' | 'I' => {
            if tokens.len() < 4 {
                return None;
            }
            let nodes = vec![tokens[1].to_string(), tokens[2].to_string()];
            let value = tokens[3..].join(" ");
            (nodes, value, None)
        }
        // Diode: D1 n+ n- model
        'D' => {
            if tokens.len() < 4 {
                return None;
            }
            let nodes = vec![tokens[1].to_string(), tokens[2].to_string()];
            let model = Some(tokens[3].to_string());
            let value = if tokens.len() > 4 {
                tokens[4..].join(" ")
            } else {
                String::new()
            };
            (nodes, value, model)
        }
        // MOSFET: M1 drain gate source bulk model
        'M' => {
            if tokens.len() < 6 {
                return None;
            }
            let nodes = vec![
                tokens[1].to_string(), // drain
                tokens[2].to_string(), // gate
                tokens[3].to_string(), // source
                tokens[4].to_string(), // bulk
            ];
            let model = Some(tokens[5].to_string());
            let value = if tokens.len() > 6 {
                tokens[6..].join(" ")
            } else {
                String::new()
            };
            (nodes, value, model)
        }
        // BJT: Q1 collector base emitter model
        'Q' => {
            if tokens.len() < 5 {
                return None;
            }
            let nodes = vec![
                tokens[1].to_string(), // collector
                tokens[2].to_string(), // base
                tokens[3].to_string(), // emitter
            ];
            let model = Some(tokens[4].to_string());
            let value = if tokens.len() > 5 {
                tokens[5..].join(" ")
            } else {
                String::new()
            };
            (nodes, value, model)
        }
        // JFET: J1 drain gate source model
        'J' => {
            if tokens.len() < 5 {
                return None;
            }
            let nodes = vec![
                tokens[1].to_string(),
                tokens[2].to_string(),
                tokens[3].to_string(),
            ];
            let model = Some(tokens[4].to_string());
            let value = if tokens.len() > 5 {
                tokens[5..].join(" ")
            } else {
                String::new()
            };
            (nodes, value, model)
        }
        // Everything else: grab what we can
        _ => {
            if tokens.len() < 3 {
                return None;
            }
            // Assume at least 2 nodes after ref_des
            let nodes = vec![tokens[1].to_string(), tokens[2].to_string()];
            let value = tokens[3..].join(" ");
            (nodes, value, None)
        }
    };

    Some(Component {
        ref_des,
        comp_type,
        nodes,
        value,
        model,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_voltage_divider() {
        let nl = parse(
            "Voltage Divider\nV1 1 0 DC 10\nR1 1 2 10k\nR2 2 0 10k\n.END\n",
        );
        assert_eq!(nl.components.len(), 3);
        assert_eq!(nl.title.as_deref(), Some("Voltage Divider"));
        assert_eq!(nl.components[0].comp_type, 'V');
        assert_eq!(nl.components[1].nodes, vec!["1", "2"]);
    }

    #[test]
    fn parse_nmos_amp() {
        let nl = parse(
            "VDD 1 0 DC 5\nRD 1 2 1k\nM1 2 3 0 0 NMOS\nVin 3 0 DC 1\n",
        );
        assert_eq!(nl.components.len(), 4);
        let m1 = &nl.components[2];
        assert_eq!(m1.comp_type, 'M');
        assert_eq!(m1.nodes, vec!["2", "3", "0", "0"]);
        assert_eq!(m1.model.as_deref(), Some("NMOS"));
    }
}

//! Grid-based auto-placement: netlist → placed components.
//!
//! Strategy:
//! 1. Build node graph, identify ground/supply/signal nodes
//! 2. Identify the "spine" — the vertical voltage path from VDD through
//!    active devices to ground
//! 3. Place spine components in a vertical column (supply → load → device → ground)
//! 4. Place input sources to the left of the spine
//! 5. Route wires to connect everything

use std::collections::{HashMap, HashSet, VecDeque};

use crate::netlist::{Netlist, Component};
use crate::scene::{PlacedComponent, SchematicScene, Rect};
use crate::symbols::{self, Orient};

/// Grid spacing in scene coordinates.
const GRID_X: f64 = 120.0;
const GRID_Y: f64 = 90.0;
const PADDING: f64 = 80.0;

pub fn place(netlist: &Netlist) -> SchematicScene {
    let ctx = LayoutContext::new(netlist);
    let placed = ctx.layout();
    let bounds = compute_bounds(&placed);

    SchematicScene {
        components: placed,
        wires: Vec::new(),
        dots: Vec::new(),
        annotations: Vec::new(),
        bounds,
    }
}

struct LayoutContext<'a> {
    netlist: &'a Netlist,
    /// node_name → set of component indices connected to it
    node_comps: HashMap<&'a str, Vec<usize>>,
    /// The ground node
    ground: &'static str,
    /// Supply nodes (positive terminal of V sources to ground)
    supply_nodes: Vec<String>,
    /// Supply source component indices
    supply_sources: Vec<usize>,
    /// Non-supply sources (signal inputs)
    signal_sources: Vec<usize>,
    /// Transistor indices
    transistors: Vec<usize>,
    /// Passive indices
    passives: Vec<usize>,
}

impl<'a> LayoutContext<'a> {
    fn new(netlist: &'a Netlist) -> Self {
        let mut node_comps: HashMap<&str, Vec<usize>> = HashMap::new();
        let mut supply_nodes = Vec::new();
        let mut supply_sources = Vec::new();
        let mut signal_sources = Vec::new();
        let mut transistors = Vec::new();
        let mut passives = Vec::new();
        let ground = "0";

        for (i, comp) in netlist.components.iter().enumerate() {
            for node in &comp.nodes {
                node_comps.entry(node.as_str()).or_default().push(i);
            }
            match comp.comp_type {
                'V' | 'I' => {
                    // A source with one terminal on ground and value looks like DC supply
                    let has_ground = comp.nodes.iter().any(|n| n == "0");
                    let is_supply = has_ground && is_supply_value(&comp.value);
                    if is_supply {
                        supply_sources.push(i);
                        if let Some(pos_node) = comp.nodes.iter().find(|n| n.as_str() != "0") {
                            supply_nodes.push(pos_node.clone());
                        }
                    } else {
                        signal_sources.push(i);
                    }
                }
                'M' | 'Q' | 'J' => transistors.push(i),
                _ => passives.push(i),
            }
        }

        Self {
            netlist,
            node_comps,
            ground,
            supply_nodes,
            supply_sources,
            signal_sources,
            transistors,
            passives,
        }
    }

    fn layout(&self) -> Vec<PlacedComponent> {
        let n = self.netlist.components.len();
        let mut grid: Vec<Option<(i32, i32, bool)>> = vec![None; n]; // (col, row, rotated)
        let mut occupied: HashSet<(i32, i32)> = HashSet::new();

        // The "spine" column is col=1 (col=0 is for input sources)
        let spine_col = 1;

        // ─── Phase 1: Build the vertical spine ───
        // For each transistor, find the path: VDD → load_resistor → drain → source → GND
        // and lay it out as a vertical column.

        let mut spine_row = 0i32;

        for &ti in &self.transistors {
            let tc = &self.netlist.components[ti];

            // Find the drain node and source node
            let (drain_node, gate_node, source_node) = match tc.comp_type {
                'M' => {
                    if tc.nodes.len() >= 3 {
                        (tc.nodes[0].as_str(), tc.nodes[1].as_str(), tc.nodes[2].as_str())
                    } else { continue; }
                }
                'Q' => {
                    if tc.nodes.len() >= 3 {
                        (tc.nodes[0].as_str(), tc.nodes[1].as_str(), tc.nodes[2].as_str())
                    } else { continue; }
                }
                _ => continue,
            };

            // Find the load component between a supply node and the drain
            let drain_load = self.find_component_between_nodes(
                &self.supply_nodes.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                drain_node,
                ti,
            );

            // Find the supply source for this path
            let supply_src = self.find_supply_for_node(drain_node, drain_load);

            // Place supply source at top of spine
            if let Some(si) = supply_src {
                if grid[si].is_none() {
                    grid[si] = Some((spine_col, spine_row, false));
                    occupied.insert((spine_col, spine_row));
                    spine_row += 1;
                }
            }

            // Place drain load (resistor) below supply — rotated vertical
            if let Some(li) = drain_load {
                if grid[li].is_none() {
                    grid[li] = Some((spine_col, spine_row, true));
                    occupied.insert((spine_col, spine_row));
                    spine_row += 1;
                }
            }

            // Place transistor below load
            grid[ti] = Some((spine_col, spine_row, false));
            occupied.insert((spine_col, spine_row));
            spine_row += 1;

            // Find source-side components (e.g., source resistor to ground)
            let source_load = self.find_component_between_node_and_ground(source_node, ti);
            if let Some(sli) = source_load {
                if grid[sli].is_none() {
                    grid[sli] = Some((spine_col, spine_row, true));
                    occupied.insert((spine_col, spine_row));
                    spine_row += 1;
                }
            }

            // Place gate input source to the left
            let gate_source = self.find_source_driving_node(gate_node);
            if let Some(gi) = gate_source {
                if grid[gi].is_none() {
                    // Place at the same row as the transistor, one column left
                    let transistor_row = grid[ti].unwrap().1;
                    let input_pos = find_free(&occupied, 0, transistor_row);
                    grid[gi] = Some((input_pos.0, input_pos.1, false));
                    occupied.insert(input_pos);
                }
            }
        }

        // ─── Phase 2: Place remaining supply sources ───
        for &si in &self.supply_sources {
            if grid[si].is_some() { continue; }
            let pos = find_free(&occupied, spine_col, 0);
            grid[si] = Some((pos.0, pos.1, false));
            occupied.insert(pos);
            spine_row = spine_row.max(pos.1 + 1);
        }

        // ─── Phase 3: Place remaining signal sources ───
        for &si in &self.signal_sources {
            if grid[si].is_some() { continue; }
            let pos = find_free(&occupied, 0, spine_row);
            grid[si] = Some((pos.0, pos.1, false));
            occupied.insert(pos);
        }

        // ─── Phase 4: Place remaining passives ───
        // For passives not yet placed (not part of a transistor spine),
        // try to place them near their connected components.

        // First: series chains (e.g., voltage divider: R1-R2 between supply and ground)
        let mut passive_col = spine_col;
        for &pi in &self.passives {
            if grid[pi].is_some() { continue; }

            let comp = &self.netlist.components[pi];

            // Find if any neighbor is already placed
            let neighbor_pos = self.find_nearest_placed_neighbor(pi, &grid);

            if let Some((ncol, nrow)) = neighbor_pos {
                // Place near the neighbor — same column, next row (vertical chain)
                // or next column if same row is taken
                let pos = find_free(&occupied, ncol, nrow + 1);
                let rotated = self.should_rotate(comp, ncol, nrow, pos.0, pos.1);
                grid[pi] = Some((pos.0, pos.1, rotated));
                occupied.insert(pos);
            } else {
                // No neighbor placed yet — put in the next available spot
                let pos = find_free(&occupied, passive_col, 0);
                grid[pi] = Some((pos.0, pos.1, false));
                occupied.insert(pos);
            }
        }

        // ─── Phase 5: Handle any stragglers ───
        for i in 0..n {
            if grid[i].is_none() {
                let pos = find_free(&occupied, 0, 0);
                grid[i] = Some((pos.0, pos.1, false));
                occupied.insert(pos);
            }
        }

        // ─── Convert to PlacedComponents ───
        let mut placed = Vec::new();
        for (i, comp) in self.netlist.components.iter().enumerate() {
            let (col, row, rotated) = grid[i].unwrap();
            let symbol = symbols::symbol_for(comp.comp_type);
            let needs_rotation = rotated && symbol.orient == Orient::Horizontal;

            let x = PADDING + col as f64 * GRID_X;
            let y = PADDING + row as f64 * GRID_Y;

            placed.push(PlacedComponent {
                ref_des: comp.ref_des.clone(),
                symbol_id: symbol.id,
                symbol,
                x,
                y,
                rotated: needs_rotation,
                label: Some(comp.ref_des.clone()),
                value: Some(format_value(comp)).filter(|s| !s.is_empty()),
            });
        }

        placed
    }

    /// Find a passive component connected between any of `from_nodes` and `to_node`,
    /// excluding component `exclude`.
    fn find_component_between_nodes(
        &self, from_nodes: &[&str], to_node: &str, exclude: usize,
    ) -> Option<usize> {
        if let Some(comps) = self.node_comps.get(to_node) {
            for &ci in comps {
                if ci == exclude { continue; }
                let comp = &self.netlist.components[ci];
                if !matches!(comp.comp_type, 'R' | 'C' | 'L' | 'D') { continue; }
                // Check if any of its other nodes is in from_nodes
                for node in &comp.nodes {
                    if node.as_str() != to_node && from_nodes.contains(&node.as_str()) {
                        return Some(ci);
                    }
                }
            }
        }
        None
    }

    /// Find a passive between `node` and ground, excluding `exclude`.
    fn find_component_between_node_and_ground(&self, node: &str, exclude: usize) -> Option<usize> {
        self.find_component_between_nodes(&[self.ground], node, exclude)
    }

    /// Find the supply source that feeds a given drain node (possibly through a load).
    fn find_supply_for_node(&self, drain_node: &str, load: Option<usize>) -> Option<usize> {
        // Direct: supply source connects to drain_node
        for &si in &self.supply_sources {
            let comp = &self.netlist.components[si];
            if comp.nodes.iter().any(|n| n.as_str() == drain_node) {
                return Some(si);
            }
        }
        // Through load: supply source connects to the other end of the load
        if let Some(li) = load {
            let load_comp = &self.netlist.components[li];
            for node in &load_comp.nodes {
                if node.as_str() == drain_node { continue; }
                for &si in &self.supply_sources {
                    let src = &self.netlist.components[si];
                    if src.nodes.iter().any(|n| n == node) {
                        return Some(si);
                    }
                }
            }
        }
        None
    }

    /// Find a source (V or I) driving a given node.
    fn find_source_driving_node(&self, node: &str) -> Option<usize> {
        for &si in &self.signal_sources {
            let comp = &self.netlist.components[si];
            if comp.nodes.iter().any(|n| n.as_str() == node) {
                return Some(si);
            }
        }
        // Also check supply sources in case it's a dual-purpose
        for &si in &self.supply_sources {
            let comp = &self.netlist.components[si];
            if comp.nodes.iter().any(|n| n.as_str() == node) {
                return Some(si);
            }
        }
        None
    }

    /// Find the grid position of the nearest already-placed neighbor of component `idx`.
    fn find_nearest_placed_neighbor(
        &self, idx: usize, grid: &[Option<(i32, i32, bool)>],
    ) -> Option<(i32, i32)> {
        let comp = &self.netlist.components[idx];
        for node in &comp.nodes {
            if let Some(neighbors) = self.node_comps.get(node.as_str()) {
                for &ni in neighbors {
                    if ni == idx { continue; }
                    if let Some((col, row, _)) = grid[ni] {
                        return Some((col, row));
                    }
                }
            }
        }
        None
    }

    /// Decide if a two-terminal component should be rotated vertical.
    fn should_rotate(&self, _comp: &Component, _ref_col: i32, _ref_row: i32, _col: i32, _row: i32) -> bool {
        // If placed in the same column as its neighbor (vertical chain), rotate
        // For now: rotate if it's a resistor/capacitor in the spine column
        false // Keep simple for now; the spine placer handles rotation explicitly
    }
}

fn is_supply_value(value: &str) -> bool {
    let v = value.trim().to_uppercase();
    // "DC 5", "5", "DC 3.3", etc. — looks like a DC supply
    v.starts_with("DC") || v.parse::<f64>().is_ok() ||
        v.strip_prefix("DC ").and_then(|s| s.trim().parse::<f64>().ok()).is_some()
}

fn find_free(occupied: &HashSet<(i32, i32)>, target_col: i32, target_row: i32) -> (i32, i32) {
    if !occupied.contains(&(target_col, target_row)) {
        return (target_col, target_row);
    }
    for radius in 1i32..20 {
        // Prefer same column (vertical neighbors) over same row
        for dr in -radius..=radius {
            let pos = (target_col, target_row + dr);
            if pos.1 >= 0 && !occupied.contains(&pos) {
                return pos;
            }
        }
        for dc in -radius..=radius {
            for dr in -radius..=radius {
                let pos = (target_col + dc, target_row + dr);
                if pos.0 >= 0 && pos.1 >= 0 && !occupied.contains(&pos) {
                    return pos;
                }
            }
        }
    }
    (target_col, target_row)
}

fn format_value(comp: &Component) -> String {
    let v = comp.value.trim();
    if v.is_empty() { return String::new(); }
    let v = v.strip_prefix("DC ").or_else(|| v.strip_prefix("dc ")).unwrap_or(v);
    // Strip anything after whitespace for clean display (e.g., "5 PULSE(...)" → "5")
    let v = v.split_whitespace().next().unwrap_or(v);
    match comp.comp_type {
        'R' => format_with_unit(v, "Ω"),
        'C' => format_with_unit(v, "F"),
        'L' => format_with_unit(v, "H"),
        'V' => format!("{v}V"),
        'I' => format!("{v}A"),
        _ => v.to_string(),
    }
}

fn format_with_unit(val: &str, unit: &str) -> String {
    if val.ends_with(unit) || val.ends_with('F') || val.ends_with('H') || val.ends_with('Ω') {
        return val.to_string();
    }
    format!("{val}{unit}")
}

fn compute_bounds(components: &[PlacedComponent]) -> Rect {
    if components.is_empty() {
        return Rect { x: 0.0, y: 0.0, w: 300.0, h: 200.0 };
    }
    let mut max_x: f64 = 0.0;
    let mut max_y: f64 = 0.0;
    for c in components {
        let b = c.bounds();
        max_x = max_x.max(b.x + b.w);
        max_y = max_y.max(b.y + b.h);
    }
    Rect { x: 0.0, y: 0.0, w: max_x + PADDING, h: max_y + PADDING }
}

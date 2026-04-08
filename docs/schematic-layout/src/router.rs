//! Orthogonal wire routing between placed component terminals.
//!
//! For each net (set of connected terminals), route wires between all
//! terminals on that net using L-shaped or Z-shaped orthogonal paths.

use std::collections::HashMap;

use crate::netlist::Netlist;
use crate::scene::{SchematicScene, Wire, Point};

/// Route wires between placed components based on netlist connectivity.
/// Modifies the scene in place, adding wires and junction dots.
pub fn route(scene: &SchematicScene, netlist: &Netlist) -> SchematicScene {
    let mut result = scene.clone();
    result.wires.clear();
    result.dots.clear();

    // Build net → terminal positions map
    let nets = build_net_map(scene, netlist);

    // Route each net
    for (net_name, terminals) in &nets {
        if terminals.len() < 2 {
            continue;
        }

        // For nets with 2 terminals: single wire
        // For nets with 3+: star topology from a central point, or chain
        if terminals.len() == 2 {
            let wire = route_two_terminal(&terminals[0], &terminals[1], net_name);
            result.wires.push(wire);
        } else {
            // Find the centroid for star routing
            let (wires, dots) = route_multi_terminal(terminals, net_name);
            result.wires.extend(wires);
            result.dots.extend(dots);
        }
    }

    // Recompute bounds to include wires
    result.bounds = compute_bounds_with_wires(&result);

    result
}

/// Map: net_name → list of terminal world positions.
fn build_net_map(scene: &SchematicScene, netlist: &Netlist) -> HashMap<String, Vec<Point>> {
    let mut nets: HashMap<String, Vec<Point>> = HashMap::new();

    for (i, comp) in netlist.components.iter().enumerate() {
        if i >= scene.components.len() {
            break;
        }
        let placed = &scene.components[i];

        // Map SPICE node order to terminal names based on component type
        let terminal_names = terminal_names_for(comp.comp_type);

        for (j, node) in comp.nodes.iter().enumerate() {
            if j < terminal_names.len() {
                if let Some(pos) = placed.terminal_pos(terminal_names[j]) {
                    nets.entry(node.clone()).or_default().push(pos);
                }
            }
        }
    }

    nets
}

/// Get terminal names in SPICE node order for each component type.
fn terminal_names_for(comp_type: char) -> &'static [&'static str] {
    match comp_type {
        'R' | 'C' | 'L' => &["left", "right"],
        'V' | 'I' => &["pos", "neg"],
        'D' => &["left", "right"], // anode, cathode
        'M' | 'J' => &["drain", "gate", "source"], // skip bulk for layout
        'Q' => &["collector", "base", "emitter"],
        _ => &["left", "right"],
    }
}

/// Route between exactly two terminals with an orthogonal path.
fn route_two_terminal(a: &Point, b: &Point, net: &str) -> Wire {
    let points = l_route(a, b);
    Wire {
        points,
        net: net.to_string(),
    }
}

/// Route a net with 3+ terminals using a shared bus point.
fn route_multi_terminal(terminals: &[Point], net: &str) -> (Vec<Wire>, Vec<Point>) {
    let mut wires = Vec::new();
    let mut dots = Vec::new();

    if terminals.is_empty() {
        return (wires, dots);
    }

    // Use the first terminal's position as the anchor,
    // then extend a horizontal or vertical bus line through all terminals.

    // Strategy: find if terminals are mostly aligned vertically or horizontally
    let avg_x: f64 = terminals.iter().map(|t| t.x).sum::<f64>() / terminals.len() as f64;
    let avg_y: f64 = terminals.iter().map(|t| t.y).sum::<f64>() / terminals.len() as f64;

    // Use the average position as the junction point
    let junction = Point { x: avg_x, y: avg_y };

    // Check if we should use a vertical bus or horizontal bus
    let x_spread = terminals.iter().map(|t| (t.x - avg_x).abs()).sum::<f64>();
    let y_spread = terminals.iter().map(|t| (t.y - avg_y).abs()).sum::<f64>();

    if x_spread < y_spread {
        // Vertical bus at avg_x
        let bus_x = avg_x;
        for term in terminals {
            let via = Point { x: bus_x, y: term.y };
            // Horizontal segment from terminal to bus
            if (term.x - bus_x).abs() > 1.0 {
                wires.push(Wire {
                    points: vec![*term, via],
                    net: net.to_string(),
                });
            }
        }
        // Vertical bus segment connecting all vias
        let mut ys: Vec<f64> = terminals.iter().map(|t| t.y).collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if let (Some(&min_y), Some(&max_y)) = (ys.first(), ys.last()) {
            wires.push(Wire {
                points: vec![
                    Point { x: bus_x, y: min_y },
                    Point { x: bus_x, y: max_y },
                ],
                net: net.to_string(),
            });
        }
        // Junction dots at T-connections
        if terminals.len() > 2 {
            for term in terminals {
                dots.push(Point { x: bus_x, y: term.y });
            }
        }
    } else {
        // Horizontal bus at avg_y
        let bus_y = avg_y;
        for term in terminals {
            let via = Point { x: term.x, y: bus_y };
            if (term.y - bus_y).abs() > 1.0 {
                wires.push(Wire {
                    points: vec![*term, via],
                    net: net.to_string(),
                });
            }
        }
        let mut xs: Vec<f64> = terminals.iter().map(|t| t.x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if let (Some(&min_x), Some(&max_x)) = (xs.first(), xs.last()) {
            wires.push(Wire {
                points: vec![
                    Point { x: min_x, y: bus_y },
                    Point { x: max_x, y: bus_y },
                ],
                net: net.to_string(),
            });
        }
        if terminals.len() > 2 {
            for term in terminals {
                dots.push(Point { x: term.x, y: bus_y });
            }
        }
    }

    (wires, dots)
}

/// Simple L-shaped orthogonal route between two points.
/// Goes horizontal first, then vertical.
fn l_route(from: &Point, to: &Point) -> Vec<Point> {
    if (from.y - to.y).abs() < 1.0 {
        // Same row — straight horizontal
        vec![*from, *to]
    } else if (from.x - to.x).abs() < 1.0 {
        // Same column — straight vertical
        vec![*from, *to]
    } else {
        // L-shape: horizontal then vertical
        vec![
            *from,
            Point { x: to.x, y: from.y },
            *to,
        ]
    }
}

fn compute_bounds_with_wires(scene: &SchematicScene) -> crate::scene::Rect {
    let mut max_x = scene.bounds.w;
    let mut max_y = scene.bounds.h;
    for wire in &scene.wires {
        for p in &wire.points {
            max_x = max_x.max(p.x + 40.0);
            max_y = max_y.max(p.y + 40.0);
        }
    }
    crate::scene::Rect {
        x: 0.0,
        y: 0.0,
        w: max_x,
        h: max_y,
    }
}

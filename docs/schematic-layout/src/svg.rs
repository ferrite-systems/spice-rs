//! SVG renderer — turns a SchematicScene into an SVG string.
//! Matches the visual style from spice-rs docs (copper/ink/paper theme).

use crate::scene::{SchematicScene, PlacedComponent, Wire, Point, Annotation, AnnotationKind, Dir};

// Design tokens matching the JS renderer
const WIRE_COLOR: &str = "#3b2f20";
const COMPONENT_COLOR: &str = "#b87333";
const VOLTAGE_COLOR: &str = "#4a6fa5";
const CURRENT_COLOR: &str = "#a04040";
const WIRE_WIDTH: f64 = 1.5;
const FONT_SIZE: f64 = 13.0;
const FONT_FAMILY: &str = "'Crimson Pro', Georgia, serif";

/// Render a schematic scene to a self-contained SVG string.
pub fn render(scene: &SchematicScene) -> String {
    let w = scene.bounds.w;
    let h = scene.bounds.h;

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#
    ));
    svg.push('\n');

    // Embed symbol definitions
    svg.push_str(&symbol_defs());

    // Wires layer
    svg.push_str("  <g class=\"wires\">\n");
    for wire in &scene.wires {
        render_wire(&mut svg, wire);
    }
    svg.push_str("  </g>\n");

    // Components layer
    svg.push_str("  <g class=\"components\">\n");
    for comp in &scene.components {
        render_component(&mut svg, comp);
    }
    svg.push_str("  </g>\n");

    // Junction dots
    for dot in &scene.dots {
        svg.push_str(&format!(
            r#"  <circle cx="{}" cy="{}" r="3" fill="{WIRE_COLOR}"/>"#,
            dot.x, dot.y
        ));
        svg.push('\n');
    }

    // Annotations
    for ann in &scene.annotations {
        render_annotation(&mut svg, ann);
    }

    svg.push_str("</svg>\n");
    svg
}

fn render_component(svg: &mut String, comp: &PlacedComponent) {
    let sym = comp.symbol;
    let w = sym.width;
    let h = sym.height;

    let href = format!("#{}", comp.symbol_id);
    if comp.rotated {
        svg.push_str(&format!(
            r#"  <use href="{href}" width="{w}" height="{h}" color="{COMPONENT_COLOR}" transform="translate({},{}) translate({},{}) rotate(90) translate({},{})"/>"#,
            comp.x, comp.y,
            w / 2.0, h / 2.0,
            -w / 2.0, -h / 2.0,
        ));
    } else {
        svg.push_str(&format!(
            r#"  <use href="{href}" x="{}" y="{}" width="{w}" height="{h}" color="{COMPONENT_COLOR}"/>"#,
            comp.x, comp.y,
        ));
    }
    svg.push('\n');

    // Label (ref_des)
    if let Some(label) = &comp.label {
        let lx = comp.x;
        let ly = comp.y - 6.0;
        svg.push_str(&format!(
            r#"  <text x="{lx}" y="{ly}" fill="{WIRE_COLOR}" font-size="{FONT_SIZE}" font-family="{FONT_FAMILY}" font-weight="600">{label}</text>"#,
        ));
        svg.push('\n');
    }

    // Value
    if let Some(value) = &comp.value {
        let vx = comp.x;
        let vy = comp.y - 6.0 + FONT_SIZE + 2.0;
        svg.push_str(&format!(
            r#"  <text x="{vx}" y="{vy}" fill="{COMPONENT_COLOR}" font-size="{}" font-family="{FONT_FAMILY}" font-style="italic">{value}</text>"#,
            FONT_SIZE - 1.0,
        ));
        svg.push('\n');
    }
}

fn render_wire(svg: &mut String, wire: &Wire) {
    if wire.points.len() < 2 {
        return;
    }
    let mut d = String::new();
    for (i, p) in wire.points.iter().enumerate() {
        if i == 0 {
            d.push_str(&format!("M{},{}", p.x, p.y));
        } else {
            d.push_str(&format!(" L{},{}", p.x, p.y));
        }
    }
    svg.push_str(&format!(
        r#"  <path d="{d}" fill="none" stroke="{WIRE_COLOR}" stroke-width="{WIRE_WIDTH}" stroke-linecap="round" stroke-linejoin="round"/>"#,
    ));
    svg.push('\n');
}

fn render_annotation(svg: &mut String, ann: &Annotation) {
    match &ann.kind {
        AnnotationKind::Voltage { text } => {
            svg.push_str(&format!(
                r#"  <text x="{}" y="{}" fill="{VOLTAGE_COLOR}" font-size="{FONT_SIZE}" font-family="{FONT_FAMILY}" font-weight="600">{text}</text>"#,
                ann.x, ann.y,
            ));
            svg.push('\n');
        }
        AnnotationKind::Current { text, dir } => {
            let (dx, dy) = match dir {
                Dir::Right => (8.0, 0.0),
                Dir::Left => (-8.0, 0.0),
                Dir::Down => (0.0, 8.0),
                Dir::Up => (0.0, -8.0),
            };
            // Arrow line
            svg.push_str(&format!(
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{CURRENT_COLOR}" stroke-width="1.5" stroke-linecap="round"/>"#,
                ann.x - dx, ann.y - dy, ann.x + dx, ann.y + dy,
            ));
            svg.push('\n');
            // Label
            svg.push_str(&format!(
                r#"  <text x="{}" y="{}" fill="{CURRENT_COLOR}" font-size="{}" font-family="{FONT_FAMILY}">{text}</text>"#,
                ann.x + dx + 4.0, ann.y + 4.0, FONT_SIZE - 1.0,
            ));
            svg.push('\n');
        }
        AnnotationKind::NodeLabel { text } => {
            svg.push_str(&format!(
                r#"  <text x="{}" y="{}" fill="{VOLTAGE_COLOR}" font-size="{FONT_SIZE}" font-family="{FONT_FAMILY}" font-weight="600">{text}</text>"#,
                ann.x, ann.y,
            ));
            svg.push('\n');
        }
    }
}

/// Inline SVG symbol definitions — ported from symbols.svg.
/// These are <symbol> elements that <use> references point to.
fn symbol_defs() -> String {
    // TODO: port the full symbol definitions from docs/components/symbols.svg
    // For now, use simple geometric primitives as placeholders
    let mut defs = String::from("  <defs>\n");

    // Resistor: zigzag
    defs.push_str(r#"    <symbol id="resistor" viewBox="0 0 60 24">
      <line x1="0" y1="12" x2="10" y2="12" stroke="currentColor" stroke-width="1.8"/>
      <polyline points="10,12 14,2 22,22 30,2 38,22 46,2 50,12" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/>
      <line x1="50" y1="12" x2="60" y2="12" stroke="currentColor" stroke-width="1.8"/>
    </symbol>
"#);

    // Capacitor: two parallel lines
    defs.push_str(r#"    <symbol id="capacitor" viewBox="0 0 40 24">
      <line x1="0" y1="12" x2="16" y2="12" stroke="currentColor" stroke-width="1.8"/>
      <line x1="16" y1="2" x2="16" y2="22" stroke="currentColor" stroke-width="1.8"/>
      <line x1="24" y1="2" x2="24" y2="22" stroke="currentColor" stroke-width="1.8"/>
      <line x1="24" y1="12" x2="40" y2="12" stroke="currentColor" stroke-width="1.8"/>
    </symbol>
"#);

    // Inductor: bumps
    defs.push_str(r#"    <symbol id="inductor" viewBox="0 0 60 20">
      <line x1="0" y1="16" x2="8" y2="16" stroke="currentColor" stroke-width="1.8"/>
      <path d="M8,16 A6,6 0 0,1 20,16 A6,6 0 0,1 32,16 A6,6 0 0,1 44,16 A6,6 0 0,1 52,16" fill="none" stroke="currentColor" stroke-width="1.8"/>
      <line x1="52" y1="16" x2="60" y2="16" stroke="currentColor" stroke-width="1.8"/>
    </symbol>
"#);

    // Voltage source: circle with +/-
    defs.push_str(r#"    <symbol id="voltage-source" viewBox="0 0 40 60">
      <line x1="20" y1="0" x2="20" y2="12" stroke="currentColor" stroke-width="1.8"/>
      <circle cx="20" cy="30" r="18" fill="none" stroke="currentColor" stroke-width="1.8"/>
      <line x1="20" y1="48" x2="20" y2="60" stroke="currentColor" stroke-width="1.8"/>
      <text x="20" y="26" text-anchor="middle" font-size="14" font-weight="bold" fill="currentColor">+</text>
      <text x="20" y="40" text-anchor="middle" font-size="14" font-weight="bold" fill="currentColor">−</text>
    </symbol>
"#);

    // Current source: circle with arrow
    defs.push_str(r#"    <symbol id="current-source" viewBox="0 0 40 60">
      <line x1="20" y1="0" x2="20" y2="12" stroke="currentColor" stroke-width="1.8"/>
      <circle cx="20" cy="30" r="18" fill="none" stroke="currentColor" stroke-width="1.8"/>
      <line x1="20" y1="48" x2="20" y2="60" stroke="currentColor" stroke-width="1.8"/>
      <line x1="20" y1="38" x2="20" y2="22" stroke="currentColor" stroke-width="1.8"/>
      <polyline points="16,26 20,22 24,26" fill="none" stroke="currentColor" stroke-width="1.5"/>
    </symbol>
"#);

    // Diode: triangle + bar
    defs.push_str(r#"    <symbol id="diode" viewBox="0 0 40 24">
      <line x1="0" y1="12" x2="13" y2="12" stroke="currentColor" stroke-width="1.8"/>
      <polygon points="13,4 27,12 13,20" fill="none" stroke="currentColor" stroke-width="1.8"/>
      <line x1="27" y1="4" x2="27" y2="20" stroke="currentColor" stroke-width="1.8"/>
      <line x1="27" y1="12" x2="40" y2="12" stroke="currentColor" stroke-width="1.8"/>
    </symbol>
"#);

    // NMOS: simplified
    defs.push_str(r#"    <symbol id="nmos" viewBox="0 0 44 60">
      <line x1="0" y1="30" x2="14" y2="30" stroke="currentColor" stroke-width="1.8"/>
      <line x1="14" y1="14" x2="14" y2="46" stroke="currentColor" stroke-width="1.8"/>
      <line x1="18" y1="14" x2="18" y2="22" stroke="currentColor" stroke-width="1.5"/>
      <line x1="18" y1="26" x2="18" y2="34" stroke="currentColor" stroke-width="1.5"/>
      <line x1="18" y1="38" x2="18" y2="46" stroke="currentColor" stroke-width="1.5"/>
      <line x1="18" y1="18" x2="34" y2="18" stroke="currentColor" stroke-width="1.8"/>
      <line x1="34" y1="18" x2="34" y2="0" stroke="currentColor" stroke-width="1.8"/>
      <line x1="18" y1="42" x2="34" y2="42" stroke="currentColor" stroke-width="1.8"/>
      <line x1="34" y1="42" x2="34" y2="60" stroke="currentColor" stroke-width="1.8"/>
      <line x1="18" y1="30" x2="34" y2="30" stroke="currentColor" stroke-width="1.8"/>
      <polyline points="24,34 28,30 24,26" fill="none" stroke="currentColor" stroke-width="1.2"/>
    </symbol>
"#);

    // Ground symbol
    defs.push_str(r#"    <symbol id="ground" viewBox="0 0 24 20">
      <line x1="12" y1="0" x2="12" y2="8" stroke="currentColor" stroke-width="1.8"/>
      <line x1="2" y1="8" x2="22" y2="8" stroke="currentColor" stroke-width="1.8"/>
      <line x1="5" y1="13" x2="19" y2="13" stroke="currentColor" stroke-width="1.5"/>
      <line x1="8" y1="18" x2="16" y2="18" stroke="currentColor" stroke-width="1.2"/>
    </symbol>
"#);

    // NPN BJT
    defs.push_str(r#"    <symbol id="npn" viewBox="0 0 44 60">
      <line x1="0" y1="30" x2="14" y2="30" stroke="currentColor" stroke-width="1.8"/>
      <line x1="14" y1="14" x2="14" y2="46" stroke="currentColor" stroke-width="2"/>
      <line x1="14" y1="22" x2="34" y2="8" stroke="currentColor" stroke-width="1.8"/>
      <line x1="34" y1="8" x2="34" y2="0" stroke="currentColor" stroke-width="1.8"/>
      <line x1="14" y1="38" x2="34" y2="52" stroke="currentColor" stroke-width="1.8"/>
      <line x1="34" y1="52" x2="34" y2="60" stroke="currentColor" stroke-width="1.8"/>
      <polyline points="26,48 34,52 30,44" fill="currentColor" stroke="currentColor" stroke-width="1"/>
    </symbol>
"#);

    // PNP BJT
    defs.push_str(r#"    <symbol id="pnp" viewBox="0 0 44 60">
      <line x1="0" y1="30" x2="14" y2="30" stroke="currentColor" stroke-width="1.8"/>
      <line x1="14" y1="14" x2="14" y2="46" stroke="currentColor" stroke-width="2"/>
      <line x1="14" y1="22" x2="34" y2="8" stroke="currentColor" stroke-width="1.8"/>
      <line x1="34" y1="8" x2="34" y2="0" stroke="currentColor" stroke-width="1.8"/>
      <line x1="14" y1="38" x2="34" y2="52" stroke="currentColor" stroke-width="1.8"/>
      <line x1="34" y1="52" x2="34" y2="60" stroke="currentColor" stroke-width="1.8"/>
      <polyline points="22,24 14,22 18,30" fill="currentColor" stroke="currentColor" stroke-width="1"/>
    </symbol>
"#);

    // PMOS
    defs.push_str(r#"    <symbol id="pmos" viewBox="0 0 48 60">
      <line x1="0" y1="30" x2="12" y2="30" stroke="currentColor" stroke-width="1.8"/>
      <circle cx="14" cy="30" r="2" fill="none" stroke="currentColor" stroke-width="1.2"/>
      <line x1="16" y1="14" x2="16" y2="46" stroke="currentColor" stroke-width="1.8"/>
      <line x1="20" y1="14" x2="20" y2="22" stroke="currentColor" stroke-width="1.5"/>
      <line x1="20" y1="26" x2="20" y2="34" stroke="currentColor" stroke-width="1.5"/>
      <line x1="20" y1="38" x2="20" y2="46" stroke="currentColor" stroke-width="1.5"/>
      <line x1="20" y1="18" x2="38" y2="18" stroke="currentColor" stroke-width="1.8"/>
      <line x1="38" y1="18" x2="38" y2="0" stroke="currentColor" stroke-width="1.8"/>
      <line x1="20" y1="42" x2="38" y2="42" stroke="currentColor" stroke-width="1.8"/>
      <line x1="38" y1="42" x2="38" y2="60" stroke="currentColor" stroke-width="1.8"/>
      <line x1="20" y1="30" x2="38" y2="30" stroke="currentColor" stroke-width="1.8"/>
    </symbol>
"#);

    defs.push_str("  </defs>\n");
    defs
}

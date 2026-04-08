//! Scene graph — the output of placement and routing.
//! Platform-agnostic: consumed by the SVG renderer (and eventually GPUI).

use crate::symbols::SymbolDef;

/// A fully laid-out schematic ready for rendering.
#[derive(Debug, Clone)]
pub struct SchematicScene {
    pub components: Vec<PlacedComponent>,
    pub wires: Vec<Wire>,
    pub dots: Vec<Point>,
    pub annotations: Vec<Annotation>,
    /// Bounding box of the scene (for SVG viewBox).
    pub bounds: Rect,
}

/// A component placed on the schematic.
#[derive(Debug, Clone)]
pub struct PlacedComponent {
    pub ref_des: String,
    pub symbol_id: &'static str,
    pub symbol: &'static SymbolDef,
    /// Top-left position in scene coordinates.
    pub x: f64,
    pub y: f64,
    /// Whether the symbol is rotated 90° from its native orientation.
    pub rotated: bool,
    /// Display label (ref_des or custom).
    pub label: Option<String>,
    /// Display value ("10kΩ", "DC 5V").
    pub value: Option<String>,
}

impl PlacedComponent {
    /// Get the world position of a terminal by name.
    pub fn terminal_pos(&self, name: &str) -> Option<Point> {
        let term = self.symbol.terminals.iter().find(|t| t.name == name)?;
        if self.rotated && self.symbol.orient != crate::symbols::Orient::Vertical {
            // Rotate 90° CW around symbol center
            let cx = self.symbol.width / 2.0;
            let cy = self.symbol.height / 2.0;
            let rx = -(term.y - cy) + cx;
            let ry = (term.x - cx) + cy;
            Some(Point { x: self.x + rx, y: self.y + ry })
        } else if !self.rotated && self.symbol.orient == crate::symbols::Orient::Vertical {
            Some(Point { x: self.x + term.x, y: self.y + term.y })
        } else if self.rotated {
            let cx = self.symbol.width / 2.0;
            let cy = self.symbol.height / 2.0;
            let rx = -(term.y - cy) + cx;
            let ry = (term.x - cx) + cy;
            Some(Point { x: self.x + rx, y: self.y + ry })
        } else {
            Some(Point { x: self.x + term.x, y: self.y + term.y })
        }
    }

    /// Bounding rect of this component in scene coords.
    pub fn bounds(&self) -> Rect {
        let (w, h) = if self.rotated {
            (self.symbol.height, self.symbol.width)
        } else {
            (self.symbol.width, self.symbol.height)
        };
        Rect { x: self.x, y: self.y, w, h }
    }
}

/// A wire segment (orthogonal polyline).
#[derive(Debug, Clone)]
pub struct Wire {
    pub points: Vec<Point>,
    pub net: String,
}

/// A junction dot (where 3+ wires meet).
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Bounding rectangle.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// An annotation on the schematic (voltage label, current arrow, node name).
#[derive(Debug, Clone)]
pub struct Annotation {
    pub x: f64,
    pub y: f64,
    pub kind: AnnotationKind,
}

#[derive(Debug, Clone)]
pub enum AnnotationKind {
    Voltage { text: String },
    Current { text: String, dir: Dir },
    NodeLabel { text: String },
}

#[derive(Debug, Clone, Copy)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

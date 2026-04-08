//! Symbol definitions — terminal positions, viewBox, orientation.
//! Ported from the JS SYMBOL_META in circuit-renderer.js.

/// Terminal position relative to symbol origin (in symbol-local coords).
#[derive(Debug, Clone, Copy)]
pub struct Terminal {
    pub name: &'static str,
    pub x: f64,
    pub y: f64,
}

/// Native orientation of a symbol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Orient {
    Horizontal,
    Vertical,
}

/// Symbol definition with viewBox, terminals, and native orientation.
#[derive(Debug, Clone)]
pub struct SymbolDef {
    pub id: &'static str,
    pub width: f64,
    pub height: f64,
    pub orient: Orient,
    pub terminals: &'static [Terminal],
}

// ── Two-terminal components ──────────────────────────────────────────

pub const RESISTOR: SymbolDef = SymbolDef {
    id: "resistor",
    width: 60.0,
    height: 24.0,
    orient: Orient::Horizontal,
    terminals: &[
        Terminal { name: "left", x: 0.0, y: 12.0 },
        Terminal { name: "right", x: 60.0, y: 12.0 },
    ],
};

pub const CAPACITOR: SymbolDef = SymbolDef {
    id: "capacitor",
    width: 40.0,
    height: 24.0,
    orient: Orient::Horizontal,
    terminals: &[
        Terminal { name: "left", x: 0.0, y: 12.0 },
        Terminal { name: "right", x: 40.0, y: 12.0 },
    ],
};

pub const INDUCTOR: SymbolDef = SymbolDef {
    id: "inductor",
    width: 60.0,
    height: 20.0,
    orient: Orient::Horizontal,
    terminals: &[
        Terminal { name: "left", x: 0.0, y: 16.0 },
        Terminal { name: "right", x: 60.0, y: 16.0 },
    ],
};

pub const DIODE: SymbolDef = SymbolDef {
    id: "diode",
    width: 40.0,
    height: 24.0,
    orient: Orient::Horizontal,
    terminals: &[
        Terminal { name: "left", x: 0.0, y: 12.0 },
        Terminal { name: "right", x: 40.0, y: 12.0 },
    ],
};

// ── Sources ──────────────────────────────────────────────────────────

pub const VOLTAGE_SOURCE: SymbolDef = SymbolDef {
    id: "voltage-source",
    width: 40.0,
    height: 60.0,
    orient: Orient::Vertical,
    terminals: &[
        Terminal { name: "pos", x: 20.0, y: 0.0 },
        Terminal { name: "neg", x: 20.0, y: 60.0 },
    ],
};

pub const CURRENT_SOURCE: SymbolDef = SymbolDef {
    id: "current-source",
    width: 40.0,
    height: 60.0,
    orient: Orient::Vertical,
    terminals: &[
        Terminal { name: "pos", x: 20.0, y: 0.0 },
        Terminal { name: "neg", x: 20.0, y: 60.0 },
    ],
};

// ── Three-terminal (transistors) ─────────────────────────────────────

pub const NMOS: SymbolDef = SymbolDef {
    id: "nmos",
    width: 44.0,
    height: 60.0,
    orient: Orient::Vertical,
    terminals: &[
        Terminal { name: "gate", x: 0.0, y: 30.0 },
        Terminal { name: "drain", x: 34.0, y: 0.0 },
        Terminal { name: "source", x: 34.0, y: 60.0 },
    ],
};

pub const PMOS: SymbolDef = SymbolDef {
    id: "pmos",
    width: 48.0,
    height: 60.0,
    orient: Orient::Vertical,
    terminals: &[
        Terminal { name: "gate", x: 0.0, y: 30.0 },
        Terminal { name: "drain", x: 38.0, y: 0.0 },
        Terminal { name: "source", x: 38.0, y: 60.0 },
    ],
};

pub const NPN: SymbolDef = SymbolDef {
    id: "npn",
    width: 44.0,
    height: 60.0,
    orient: Orient::Vertical,
    terminals: &[
        Terminal { name: "base", x: 0.0, y: 30.0 },
        Terminal { name: "collector", x: 34.0, y: 0.0 },
        Terminal { name: "emitter", x: 34.0, y: 60.0 },
    ],
};

pub const PNP: SymbolDef = SymbolDef {
    id: "pnp",
    width: 44.0,
    height: 60.0,
    orient: Orient::Vertical,
    terminals: &[
        Terminal { name: "base", x: 0.0, y: 30.0 },
        Terminal { name: "collector", x: 34.0, y: 60.0 },
        Terminal { name: "emitter", x: 34.0, y: 0.0 },
    ],
};

// ── Special ──────────────────────────────────────────────────────────

pub const GROUND: SymbolDef = SymbolDef {
    id: "ground",
    width: 24.0,
    height: 20.0,
    orient: Orient::Vertical,
    terminals: &[
        Terminal { name: "top", x: 12.0, y: 0.0 },
    ],
};

/// Look up the symbol definition for a component type prefix.
pub fn symbol_for(comp_type: char) -> &'static SymbolDef {
    match comp_type {
        'R' | 'r' => &RESISTOR,
        'C' | 'c' => &CAPACITOR,
        'L' | 'l' => &INDUCTOR,
        'D' | 'd' => &DIODE,
        'V' | 'v' => &VOLTAGE_SOURCE,
        'I' | 'i' => &CURRENT_SOURCE,
        'M' | 'm' => &NMOS,    // default to NMOS; placer can override for PMOS
        'Q' | 'q' => &NPN,     // default to NPN; placer can override for PNP
        'J' | 'j' => &NMOS,    // JFET uses same layout shape
        _ => &RESISTOR,         // fallback
    }
}

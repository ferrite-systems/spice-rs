/// Simulation mode flags — matches ngspice CKTmode bit definitions (cktdefs.h:169-200).
///
/// Devices check these flags in their `load()` to determine behavior.
#[derive(Debug, Clone, Copy)]
pub struct Mode {
    bits: u32,
}

// Analysis type flags — must match ngspice cktdefs.h:176-183 exactly
pub const MODETRAN: u32 = 0x1;
pub const MODEAC: u32 = 0x2;
pub const MODEDC: u32 = 0x70;         // mask: MODEDCOP | MODETRANOP | MODEDCTRANCURVE
pub const MODEDCOP: u32 = 0x10;       // DC operating point
pub const MODETRANOP: u32 = 0x20;     // DC OP for transient (sources ramped)
pub const MODEDCTRANCURVE: u32 = 0x40; // DC transfer curve (sweep)
pub const MODEUIC: u32 = 0x10000;     // Use initial conditions (cktdefs.h:186)

// Init mode flags (mutually exclusive within INITF_MASK)
pub const MODEINITJCT: u32 = 0x200;
pub const MODEINITFIX: u32 = 0x400;
pub const MODEINITFLOAT: u32 = 0x100;
pub const MODEINITTRAN: u32 = 0x1000;
pub const MODEINITPRED: u32 = 0x2000;
pub const MODEINITSMSIG: u32 = 0x800;  // Small-signal mode (cktdefs.h:190)

/// Mask covering all init flags.
pub const INITF_MASK: u32 = 0x3f00;

impl Mode {
    pub fn new(bits: u32) -> Self {
        Self { bits }
    }

    pub fn bits(self) -> u32 {
        self.bits
    }

    /// Test if a flag is set.
    pub fn is(self, flag: u32) -> bool {
        (self.bits & flag) != 0
    }

    /// Clear all init flags and set the given init flag.
    /// Matches: `ckt->CKTmode = (ckt->CKTmode & ~INITF) | new_flag`
    pub fn set_init(&mut self, init_flag: u32) {
        self.bits = (self.bits & !INITF_MASK) | init_flag;
    }
}

/// NI state flags — controls sparse solver reordering (cktdefs.h:143-150).
#[derive(Debug, Clone, Copy)]
pub struct NiState {
    bits: u32,
}

pub const NI_SHOULD_REORDER: u32 = 0x1;
pub const NI_DID_PREORDER: u32 = 0x100;
pub const NI_UNINITIALIZED: u32 = 0x4;

impl NiState {
    pub fn new() -> Self {
        Self {
            bits: NI_UNINITIALIZED,
        }
    }

    pub fn is(self, flag: u32) -> bool {
        (self.bits & flag) != 0
    }

    pub fn set(&mut self, flag: u32) {
        self.bits |= flag;
    }

    pub fn clear(&mut self, flag: u32) {
        self.bits &= !flag;
    }
}

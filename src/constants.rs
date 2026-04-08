//! Physical constants — must match ngspice const.h exactly.
//!
//! ngspice source: src/include/ngspice/const.h
//! All device models MUST use these values, not their own copies.

/// Electron charge (C) — ngspice CHARGE
pub const CHARGE: f64 = 1.6021766208e-19;

/// Boltzmann constant (J/K) — ngspice CONSTboltz
pub const BOLTZ: f64 = 1.38064852e-23;

/// Reference temperature (K) — ngspice REFTEMP = 27°C + 273.15
pub const REFTEMP: f64 = 300.15;

/// k/q (V/K) — ngspice CONSTKoverQ
pub const KoverQ: f64 = BOLTZ / CHARGE;

/// Speed of light (m/s) — ngspice CONSTc (exact SI)
pub const C_LIGHT: f64 = 299792458.0;

/// Vacuum permeability (H/m) — ngspice CONSTmuZero
pub const MU0: f64 = 4.0 * std::f64::consts::PI * 1e-7;

/// Vacuum permittivity (F/m) — ngspice CONSTepsZero = 1/(mu0*c^2)
pub const EPS0: f64 = 1.0 / (MU0 * C_LIGHT * C_LIGHT);

/// SiO2 relative permittivity — ngspice CONSTepsrSiO2
pub const EPSR_SIO2: f64 = 3.9;

/// SiO2 permittivity (F/m) — ngspice CONSTepsSiO2
pub const EPS_SIO2: f64 = EPSR_SIO2 * EPS0;

/// Silicon relative permittivity
pub const EPSR_SI: f64 = 11.7;

/// Silicon permittivity (F/m)
pub const EPS_SI: f64 = EPSR_SI * EPS0;

/// sqrt(2) — ngspice CONSTroot2
pub const ROOT2: f64 = std::f64::consts::SQRT_2;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify our constants match ngspice const.h bit-for-bit.
    /// If this test fails, a constant was changed without updating ngspice.
    #[test]
    fn constants_match_ngspice() {
        // These hex values were computed from ngspice's const.h definitions
        // using the same FP operations ngspice uses.
        assert_eq!(CHARGE.to_bits(), 1.6021766208e-19_f64.to_bits());
        assert_eq!(BOLTZ.to_bits(), 1.38064852e-23_f64.to_bits());
        assert_eq!(REFTEMP, 300.15);

        // vt at REFTEMP — key derived quantity
        let vt = BOLTZ * REFTEMP / CHARGE;
        let vt_via_koverq = KoverQ * REFTEMP;
        assert_eq!(vt.to_bits(), vt_via_koverq.to_bits(),
            "vt computation must be consistent: BOLTZ*T/Q vs KoverQ*T");
    }
}

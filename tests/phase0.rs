//! Phase 0 validation: single resistor circuit against expected values.
//!
//! Circuit: V1 (5V) -- R1 (1kΩ) -- GND
//!
//! Expected: V(1) = 5.0V, I(V1) = -5mA (convention: current into + terminal)

use spice_rs::analysis::dc::dc_operating_point;
use spice_rs::circuit::Circuit;
use spice_rs::config::SimConfig;
use spice_rs::device::resistor::Resistor;
use spice_rs::device::vsource::VoltageSource;

#[test]
fn single_resistor_dc_op() {
    let mut circuit = Circuit::new();

    // Create nodes: "1" is the junction between V1+ and R1+
    let n1 = circuit.node("1");
    let gnd = 0; // ground is always equation 0

    // V1: 5V from node 1 to ground, needs a branch equation
    let v1_branch = circuit.branch("v1#branch");
    circuit.add_device(Box::new(VoltageSource::new("V1", n1, gnd, v1_branch, 5.0)));

    // R1: 1kΩ from node 1 to ground
    circuit.add_device(Box::new(Resistor::new("R1", n1, gnd, 1000.0)));

    // Setup
    circuit.setup();
    let config = SimConfig::default();
    circuit.temperature(&config);

    // Solve DC OP
    let sim = dc_operating_point(&mut circuit, &config).expect("DC OP should converge");

    eprintln!("rhs={:?}", &sim.mna.rhs[..3]);
    eprintln!("rhs_old={:?}", &sim.mna.rhs_old[..3]);
    eprintln!("n1={n1} v1_branch={v1_branch}");

    // Verify: V(1) = 5.0V
    let v1 = sim.mna.rhs_old[n1];
    assert!(
        (v1 - 5.0).abs() < 1e-12,
        "V(1) = {v1}, expected 5.0"
    );

    // Verify: I(V1) = -5mA (current flows out of + terminal into circuit)
    let i_v1 = sim.mna.rhs_old[v1_branch];
    assert!(
        (i_v1 - (-0.005)).abs() < 1e-12,
        "I(V1) = {i_v1}, expected -0.005"
    );
}

#[test]
fn resistor_divider_dc_op() {
    // V1 (10V) -- R1 (1kΩ) -- mid -- R2 (1kΩ) -- GND
    // Expected: V(mid) = 5.0V

    let mut circuit = Circuit::new();

    let n_top = circuit.node("top");
    let n_mid = circuit.node("mid");
    let gnd = 0;

    let v1_branch = circuit.branch("v1#branch");
    circuit.add_device(Box::new(VoltageSource::new("V1", n_top, gnd, v1_branch, 10.0)));
    circuit.add_device(Box::new(Resistor::new("R1", n_top, n_mid, 1000.0)));
    circuit.add_device(Box::new(Resistor::new("R2", n_mid, gnd, 1000.0)));

    circuit.setup();
    let config = SimConfig::default();
    circuit.temperature(&config);

    let sim = dc_operating_point(&mut circuit, &config).expect("DC OP should converge");

    let v_top = sim.mna.rhs_old[n_top];
    let v_mid = sim.mna.rhs_old[n_mid];
    let i_v1 = sim.mna.rhs_old[v1_branch];

    assert!(
        (v_top - 10.0).abs() < 1e-12,
        "V(top) = {v_top}, expected 10.0"
    );
    assert!(
        (v_mid - 5.0).abs() < 1e-12,
        "V(mid) = {v_mid}, expected 5.0"
    );
    assert!(
        (i_v1 - (-0.005)).abs() < 1e-12,
        "I(V1) = {i_v1}, expected -0.005"
    );
}

#[test]
fn three_resistor_network() {
    // V1 (12V) -- R1 (1kΩ) -- a -- R2 (2kΩ) -- b -- R3 (3kΩ) -- GND
    // Total R = 6kΩ, I = 2mA
    // V(a) = 12 - 1k*2m = 10V
    // V(b) = 12 - 3k*2m = 6V

    let mut circuit = Circuit::new();

    let n_top = circuit.node("top");
    let n_a = circuit.node("a");
    let n_b = circuit.node("b");
    let gnd = 0;

    let v1_br = circuit.branch("v1#branch");
    circuit.add_device(Box::new(VoltageSource::new("V1", n_top, gnd, v1_br, 12.0)));
    circuit.add_device(Box::new(Resistor::new("R1", n_top, n_a, 1000.0)));
    circuit.add_device(Box::new(Resistor::new("R2", n_a, n_b, 2000.0)));
    circuit.add_device(Box::new(Resistor::new("R3", n_b, gnd, 3000.0)));

    circuit.setup();
    let config = SimConfig::default();
    circuit.temperature(&config);

    let sim = dc_operating_point(&mut circuit, &config).expect("DC OP should converge");

    let v_a = sim.mna.rhs_old[n_a];
    let v_b = sim.mna.rhs_old[n_b];

    assert!(
        (v_a - 10.0).abs() < 1e-9,
        "V(a) = {v_a}, expected 10.0"
    );
    assert!(
        (v_b - 6.0).abs() < 1e-9,
        "V(b) = {v_b}, expected 6.0"
    );
}

pub mod netlist;
pub mod symbols;
pub mod scene;
pub mod placer;
pub mod router;
pub mod svg;

/// Take a SPICE netlist string, auto-layout, return SVG string.
pub fn netlist_to_svg(netlist: &str) -> String {
    let parsed = netlist::parse(netlist);
    let placed = placer::place(&parsed);
    let routed = router::route(&placed, &parsed);
    svg::render(&routed)
}

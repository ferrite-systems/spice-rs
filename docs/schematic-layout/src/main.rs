use std::io::{self, Read};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let netlist = if args.len() > 1 {
        std::fs::read_to_string(&args[1]).expect("Failed to read netlist file")
    } else {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).expect("Failed to read stdin");
        buf
    };

    let svg = schematic_layout::netlist_to_svg(&netlist);
    println!("{svg}");
}

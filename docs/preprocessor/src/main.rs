//! mdbook preprocessor: transforms ```spice-sim fenced blocks into interactive
//! simulation widgets powered by spice-rs WASM.
//!
//! Input format (in markdown):
//! ```spice-sim
//! analysis: tran
//! outputs: v(out)
//! editable: R1, C1
//! ---
//! V1 in 0 DC 0 PULSE(0 5 0 1n 1n 0.5m 1m)
//! R1 in out 1k
//! C1 out 0 1u
//! .TRAN 5m
//! .END
//! ```
//!
//! Output: HTML div.spice-sim-container with textarea, run button, and output area.

use std::io::{self, Read};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // mdbook calls preprocessors with "supports <renderer>" to check compatibility
    if args.len() >= 3 && args[1] == "supports" {
        if args[2] == "html" {
            std::process::exit(0);
        } else {
            std::process::exit(1);
        }
    }

    // Read [context, book] from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).expect("Failed to read stdin");

    let input_val: serde_json::Value = serde_json::from_str(&input).expect("Failed to parse book JSON");

    let (_context, mut book) = match input_val {
        serde_json::Value::Array(mut arr) if arr.len() == 2 => {
            let book = arr.remove(1);
            let ctx = arr.remove(0);
            (ctx, book)
        }
        _ => panic!("Expected [context, book] array from mdbook"),
    };

    // Walk all items/sections in the book and transform spice-sim blocks.
    // mdbook v0.5+ uses "items", older versions use "sections".
    let mut count = 0;
    for key in &["items", "sections"] {
        if let Some(items) = book.get_mut(*key) {
            if let Some(arr) = items.as_array_mut() {
                for item in arr.iter_mut() {
                    count += process_item(item);
                }
            }
        }
    }
    eprintln!("[spice-sim] Transformed {count} blocks");

    let output = serde_json::to_string(&book).expect("Failed to serialize book JSON");
    print!("{}", output);
}

/// Recursively process a book item (Chapter or separator).
fn process_item(item: &mut serde_json::Value) -> usize {
    let mut count = 0;

    // Handle Chapter variant
    if let Some(chapter) = item.get_mut("Chapter") {
        // Transform content
        if let Some(content) = chapter.get_mut("content") {
            if let Some(s) = content.as_str() {
                let mut transformed = s.to_string();
                if transformed.contains("```spice-sim") {
                    transformed = transform_spice_blocks(&transformed);
                    count += transformed.matches("spice-sim-container").count();
                }
                if transformed.contains("```ferrite-circuit") {
                    let (new_content, circuit_count) = transform_circuit_blocks(&transformed);
                    transformed = new_content;
                    count += circuit_count;
                }
                if count > 0 {
                    *content = serde_json::Value::String(transformed);
                }
            }
        }
        // Recurse into sub_items (old) and chapters (new)
        for key in &["sub_items", "chapters"] {
            if let Some(subs) = chapter.get_mut(*key) {
                if let Some(arr) = subs.as_array_mut() {
                    for sub in arr.iter_mut() {
                        count += process_item(sub);
                    }
                }
            }
        }
    }

    count
}

fn transform_spice_blocks(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut pos = 0;

    while pos < content.len() {
        if let Some(block_start) = content[pos..].find("```spice-sim") {
            let abs_start = pos + block_start;
            result.push_str(&content[pos..abs_start]);

            let after_open = abs_start + "```spice-sim".len();
            if let Some(block_end) = content[after_open..].find("\n```") {
                let abs_end = after_open + block_end + "\n```".len();
                let block_content = content[after_open..after_open + block_end].trim_start_matches('\n');

                let (analysis, netlist, autorun) = parse_block(block_content);
                result.push_str(&emit_widget(&analysis, &netlist, autorun));

                pos = abs_end;
                if pos < content.len() && content.as_bytes()[pos] == b'\n' {
                    pos += 1;
                }
            } else {
                result.push_str("```spice-sim");
                pos = after_open;
            }
        } else {
            result.push_str(&content[pos..]);
            break;
        }
    }

    result
}

fn parse_block(block: &str) -> (String, String, bool) {
    let mut analysis = "dc".to_string();
    let mut autorun = false;

    if let Some(sep_pos) = block.find("\n---\n") {
        let header = &block[..sep_pos];
        let netlist = block[sep_pos + "\n---\n".len()..].to_string();

        for line in header.lines() {
            let line = line.trim();
            if let Some((key, val)) = line.split_once(':') {
                match key.trim() {
                    "analysis" => analysis = val.trim().to_string(),
                    "autorun" => autorun = val.trim() == "true",
                    _ => {}
                }
            }
        }

        (analysis, netlist, autorun)
    } else {
        (analysis, block.to_string(), autorun)
    }
}

fn emit_widget(analysis: &str, netlist: &str, autorun: bool) -> String {
    let autorun_attr = if autorun { " data-autorun=\"true\"" } else { "" };
    let escaped_netlist = netlist
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");

    format!(
        r#"<div class="spice-sim-container" data-analysis="{analysis}"{autorun_attr}>
  <div class="netlist-editor">
    <textarea>{escaped_netlist}</textarea>
    <button class="run-btn">Run</button>
  </div>
  <div class="sim-output"></div>
</div>"#
    )
}

// ── ferrite-circuit blocks → client-side KDL widget container ────────
//
// The preprocessor no longer renders SVG at build time. It embeds the raw
// KDL in a <script type="application/kdl"> tag. The JS runtime loads WASM
// and renders the SVG, param controls, and simulation results client-side.

fn transform_circuit_blocks(content: &str) -> (String, usize) {
    let mut result = String::with_capacity(content.len());
    let mut count = 0;
    let mut pos = 0;

    while pos < content.len() {
        if let Some(block_start) = content[pos..].find("```ferrite-circuit") {
            let abs_start = pos + block_start;
            result.push_str(&content[pos..abs_start]);

            let after_open = abs_start + "```ferrite-circuit".len();
            if let Some(block_end) = content[after_open..].find("\n```") {
                let abs_end = after_open + block_end + "\n```".len();
                let kdl = content[after_open..after_open + block_end].trim();

                let widget_id = format!("ferrite-circuit-{count}");
                let escaped_kdl = kdl
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");

                result.push_str(&format!(
                    r#"<div class="ferrite-circuit-widget" id="{widget_id}">
  <script type="application/kdl">{escaped_kdl}</script>
  <div class="ferrite-circuit-svg"><div class="ferrite-loading">Loading circuit…</div></div>
  <div class="ferrite-circuit-panel">
    <div class="ferrite-circuit-controls"></div>
    <div class="ferrite-circuit-actions"></div>
    <div class="ferrite-circuit-netlist"></div>
  </div>
</div>
"#));
                count += 1;
                eprintln!("[ferrite-circuit] Embedded KDL widget");

                pos = abs_end;
                if pos < content.len() && content.as_bytes()[pos] == b'\n' {
                    pos += 1;
                }
            } else {
                result.push_str("```ferrite-circuit");
                pos = after_open;
            }
        } else {
            result.push_str(&content[pos..]);
            break;
        }
    }

    (result, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_simple_block() {
        let input = "Some text before.\n\n```spice-sim\nanalysis: dc\n---\nV1 in 0 DC 5\nR1 in 0 1k\n.OP\n.END\n```\n\nSome text after.\n";
        let output = transform_spice_blocks(input);
        assert!(output.contains("spice-sim-container"));
        assert!(output.contains("data-analysis=\"dc\""));
        assert!(output.contains("V1 in 0 DC 5"));
        assert!(output.contains("Some text before."));
        assert!(output.contains("Some text after."));
        assert!(!output.contains("```spice-sim"));
    }

    #[test]
    fn test_no_header() {
        let input = "```spice-sim\nV1 in 0 DC 5\n.OP\n.END\n```";
        let output = transform_spice_blocks(input);
        assert!(output.contains("data-analysis=\"dc\""));
        assert!(output.contains("V1 in 0 DC 5"));
    }

    #[test]
    fn test_circuit_block() {
        let input = r#"Text before.

```ferrite-circuit
circuit "Test" {
    node "vdd" label="VDD" rail=#true voltage="5"
    node "gnd" ground=#true
    group "load" topology="generic" {
        component "R1" type="resistor" role="passive" {
            value "1k"
            port "1" net="vdd"
            port "2" net="gnd"
        }
    }
}
```

Text after.
"#;
        let (output, count) = transform_circuit_blocks(input);
        assert_eq!(count, 1);
        assert!(output.contains("ferrite-circuit-widget"), "missing widget container");
        assert!(output.contains("application/kdl"), "missing KDL script tag");
        assert!(output.contains("ferrite-circuit-svg"), "missing SVG container");
        assert!(output.contains("Text before."));
        assert!(output.contains("Text after."));
        assert!(!output.contains("```ferrite-circuit"));
    }

    #[test]
    fn test_autorun() {
        let input = "```spice-sim\nanalysis: tran\nautorun: true\n---\nV1 in 0 PULSE(0 5 0 1n 1n 0.5m 1m)\nR1 in out 1k\nC1 out 0 1u\n.TRAN 5m\n.END\n```";
        let output = transform_spice_blocks(input);
        assert!(output.contains("data-autorun=\"true\""));
        assert!(output.contains("data-analysis=\"tran\""));
    }
}

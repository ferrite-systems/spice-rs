# SPICE Netlist Syntax

SPICE netlists are plain-text files that describe a circuit's components, connections, and analysis commands. spice-rs accepts standard SPICE3-compatible netlists.

A netlist has three sections:

1. **Title line** -- the first line of the file (always treated as a comment)
2. **Circuit description** -- device instances, model definitions, subcircuits
3. **Analysis commands** -- what simulation to run

```spice
Simple RC Circuit
R1 in out 1k
C1 out 0 1u
V1 in 0 DC 5
.OP
.END
```

## In this chapter

- [Format](ch13-01-format.md) -- file structure, comments, continuations, number suffixes
- [Device Statements](ch13-02-devices.md) -- syntax for each supported device type
- [Analysis Commands](ch13-03-analysis.md) -- `.OP`, `.DC`, `.AC`, `.TRAN`, and more
- [Control Statements](ch13-04-control.md) -- `.MODEL`, `.SUBCKT`, `.OPTIONS`, `.PARAM`, etc.

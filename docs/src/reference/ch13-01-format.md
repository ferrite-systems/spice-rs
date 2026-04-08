# Netlist Format

## File structure

| Rule | Details |
|------|---------|
| Title line | First line of the file. Always treated as a comment. |
| Comments | Lines starting with `*` are ignored. |
| End marker | `.END` marks the end of the netlist. Everything after is ignored. |
| Continuation | Lines starting with `+` are appended to the previous line. |
| Ground node | Node `0` (or `GND`) is the global reference (ground). Every circuit must have at least one connection to node `0`. |
| Case | Case insensitive. `R1`, `r1`, and `R1` are the same element. |

## Comments

```spice
* This entire line is a comment
R1 in out 1k   $ Inline comments use $ (ngspice extension)
```

## Line continuation

Long lines can be split with `+`:

```spice
M1 drain gate source bulk NMOS
+ W=10u L=0.18u
+ AD=5p AS=5p
```

This is equivalent to a single line:

```
M1 drain gate source bulk NMOS W=10u L=0.18u AD=5p AS=5p
```

## Number suffixes

Numeric values accept standard engineering suffixes:

| Suffix | Multiplier | Example |
|--------|-----------|---------|
| `T`    | 1e12      | `1T` = 1e12 |
| `G`    | 1e9       | `2.2G` = 2.2e9 |
| `MEG`  | 1e6       | `4.7MEG` = 4.7e6 |
| `K`    | 1e3       | `10K` = 1e4 |
| `M`    | 1e-3      | `5M` = 5e-3 |
| `MIL`  | 25.4e-6   | `1MIL` = 25.4e-6 |
| `U`    | 1e-6      | `100U` = 1e-4 |
| `N`    | 1e-9      | `10N` = 1e-8 |
| `P`    | 1e-12     | `22P` = 2.2e-11 |
| `F`    | 1e-15     | `1F` = 1e-15 |

Suffixes are case insensitive. Trailing alphabetic characters after a recognized suffix are ignored, so `10uF` parses as `10e-6` (the `F` is not a femto suffix because `u` was already matched).

Scientific notation is also accepted: `1.5e-3`, `2.0E6`.

## Node names

Nodes can be numeric (`1`, `2`, `3`) or alphanumeric (`in`, `out`, `Vdd`). Node `0` is always ground.

## Example

```spice
Voltage Divider
* Supply
V1 vdd 0 DC 3.3
* Resistors
R1 vdd mid 10K
R2 mid 0 10K
.OP
.END
```

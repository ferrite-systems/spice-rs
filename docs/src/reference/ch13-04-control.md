# Control Statements

Control statements configure models, subcircuits, parameters, and simulation behavior.

## .MODEL -- Device Model Definition

```
.MODEL name type (param=val ...)
```

Defines a set of model parameters for a device type.

| Type | Device |
|------|--------|
| `D`    | Diode |
| `NPN`  | NPN BJT |
| `PNP`  | PNP BJT |
| `NMOS` | N-channel MOSFET |
| `PMOS` | P-channel MOSFET |
| `NJF`  | N-channel JFET |
| `PJF`  | P-channel JFET |

```spice
.MODEL DMOD D (IS=1e-14 N=1.05 RS=10 CJO=2P)
.MODEL NMOS NMOS (LEVEL=1 VTO=0.7 KP=110U GAMMA=0.4)
.MODEL NPN1 NPN (IS=1e-15 BF=200 VAF=100)
```

For MOSFET models, the `LEVEL` parameter selects the model:

| Level | Model |
|-------|-------|
| 1     | Shichman-Hodges (MOS1) |
| 2     | Grove-Frohman (MOS2) |
| 3     | Semi-empirical (MOS3) |
| 8     | BSIM3v3 |
| 14    | BSIM4 |

## .SUBCKT / .ENDS -- Subcircuit Definition

```
.SUBCKT name node1 node2 ...
  ... circuit description ...
.ENDS [name]
```

Defines a reusable subcircuit. Internal nodes are local. Instantiate with `X`:

```
Xname node1 node2 ... subckt_name
```

```spice
.SUBCKT INV in out vdd vss
M1 out in vdd vdd PMOS W=2U L=0.18U
M2 out in vss vss NMOS W=1U L=0.18U
.ENDS INV

X1 a y vdd 0 INV
```

## .PARAM -- Parameter Definition

```
.PARAM name=expression
```

Defines a named parameter that can be used in device values and expressions.

```spice
.PARAM vdd_val=3.3
.PARAM rload=10K
V1 vdd 0 DC {vdd_val}
R1 out 0 {rload}
```

## .OPTIONS -- Simulation Options

```
.OPTIONS key=value ...
```

Controls simulation accuracy, convergence, and behavior. See [Chapter 15: Simulation Options](ch15-options.md) for the full list.

```spice
.OPTIONS RELTOL=1e-4 ABSTOL=1e-14 TEMP=85
```

## .INCLUDE -- File Inclusion

```
.INCLUDE "filename"
```

Inserts the contents of another file at this point in the netlist.

```spice
.INCLUDE "models/nmos.mod"
```

## .LIB -- Library Inclusion

```
.LIB "filename" section
```

Includes a named section from a library file. Library files use `.LIB section` / `.ENDL section` delimiters.

```spice
.LIB "cmos.lib" TT
```

Library file format:

```spice
.LIB TT
.MODEL NMOS NMOS (VTO=0.5 ...)
.MODEL PMOS PMOS (VTO=-0.5 ...)
.ENDL TT

.LIB FF
.MODEL NMOS NMOS (VTO=0.4 ...)
.MODEL PMOS PMOS (VTO=-0.4 ...)
.ENDL FF
```

## .IC -- Initial Conditions

```
.IC V(node)=val ...
```

Forces specific node voltages as initial conditions for transient analysis. These are applied before the DC operating point when used without `UIC`, or directly as starting values with `UIC`.

```spice
.IC V(out)=0 V(vdd)=3.3
```

## .NODESET -- DC Operating Point Hints

```
.NODESET V(node)=val ...
```

Provides an initial guess for the DC operating point solver. Unlike `.IC`, these are hints -- the solver can move away from them. Useful for helping convergence in circuits with multiple stable states (e.g., latches, oscillators).

```spice
.NODESET V(q)=3.3 V(qbar)=0
```

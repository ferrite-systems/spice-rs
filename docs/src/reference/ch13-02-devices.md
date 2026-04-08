# Device Statements

Each device instance is a single line (or continued with `+`). The first letter of the name determines the device type.

## Resistor (R)

```
Rname n+ n- value [ac=acval]
```

`ac=acval` sets a separate resistance used only during AC analysis.

```spice
R1 in out 4.7K
Rload out 0 50 ac=100
```

## Capacitor (C)

```
Cname n+ n- value [ic=v0]
```

`ic=v0` sets the initial voltage across the capacitor (used with `.TRAN ... UIC`).

```spice
C1 out 0 100N
Cbypass vdd 0 10U ic=3.3
```

## Inductor (L)

```
Lname n+ n- value [ic=i0]
```

`ic=i0` sets the initial current through the inductor.

```spice
L1 in out 10U
Lchoke supply filtered 100U ic=0.5
```

## Coupled Inductors (K)

```
Kname Lname1 Lname2 coupling
```

`coupling` is the coupling coefficient (0 to 1).

```spice
L1 in 0 10U
L2 out 0 10U
K1 L1 L2 0.99
```

## Transmission Line (T)

```
Tname n1 n2 n3 n4 Z0=val TD=val
```

Lossless transmission line. `n1`/`n2` are port 1, `n3`/`n4` are port 2.

```spice
T1 in 0 out 0 Z0=50 TD=1N
```

## Voltage Source (V)

```
Vname n+ n- [DC val] [AC mag [phase]] [transient_spec]
```

Transient specifications:

| Type | Syntax |
|------|--------|
| Pulse | `PULSE(v1 v2 td tr tf pw per)` |
| Sine | `SIN(vo va freq td theta)` |
| Exponential | `EXP(v1 v2 td1 tau1 td2 tau2)` |
| Piece-wise linear | `PWL(t1 v1 t2 v2 ...)` |

```spice
V1 vdd 0 DC 5
Vac in 0 AC 1 0
Vclk clk 0 PULSE(0 3.3 0 1N 1N 5U 10U)
Vsin sig 0 SIN(0 1 1K)
```

## Current Source (I)

```
Iname n+ n- [DC val] [AC mag [phase]] [transient_spec]
```

Same transient specifications as voltage sources. Current flows from `n+` through the source to `n-`.

```spice
I1 0 bias DC 100U
Iac 0 in AC 1M
```

## Diode (D)

```
Dname n+ n- modelname [area]
```

`n+` is the anode, `n-` is the cathode.

```spice
D1 in out DMOD
.MODEL DMOD D (IS=1e-14 N=1.05 RS=10)
```

## MOSFET (M)

```
Mname drain gate source bulk modelname [W=val] [L=val] [M=val]
+ [AD=val] [AS=val] [PD=val] [PS=val] [NRD=val] [NRS=val]
```

`M=val` is the multiplier (number of parallel devices).

```spice
M1 out in vdd vdd PMOS W=10U L=0.18U
M2 out in 0 0 NMOS W=5U L=0.18U M=2
.MODEL NMOS NMOS (VTO=0.7 KP=110U)
.MODEL PMOS PMOS (VTO=-0.7 KP=50U)
```

## BJT (Q)

```
Qname collector base emitter [substrate] modelname [area]
```

If `substrate` is omitted, it defaults to ground.

```spice
Q1 out base 0 NPN1
Q2 out base emitter sub PNP1 2.0
.MODEL NPN1 NPN (IS=1e-15 BF=200)
.MODEL PNP1 PNP (IS=1e-15 BF=100)
```

## JFET (J)

```
Jname drain gate source modelname [area]
```

```spice
J1 out gate 0 JMOD
.MODEL JMOD NJF (VTO=-2 BETA=1e-4)
```

## VCVS -- Voltage-Controlled Voltage Source (E)

```
Ename n+ n- nc+ nc- gain
```

Output voltage = gain * V(nc+, nc-).

```spice
E1 out 0 in 0 10
```

## VCCS -- Voltage-Controlled Current Source (G)

```
Gname n+ n- nc+ nc- gain
```

Output current = gain * V(nc+, nc-). Current flows from `n+` to `n-`.

```spice
G1 out 0 in 0 0.001
```

## CCVS -- Current-Controlled Voltage Source (H)

```
Hname n+ n- vcontrol gain
```

Output voltage = gain * I(vcontrol). `vcontrol` is the name of a voltage source whose current is the controlling variable.

```spice
Vsense in mid 0
H1 out 0 Vsense 1K
```

## CCCS -- Current-Controlled Current Source (F)

```
Fname n+ n- vcontrol gain
```

Output current = gain * I(vcontrol).

```spice
Vsense in mid 0
F1 out 0 Vsense 100
```

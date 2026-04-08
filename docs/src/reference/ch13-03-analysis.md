# Analysis Commands

Each netlist contains one or more analysis commands that tell the simulator what to compute.

## .OP -- DC Operating Point

```
.OP
```

Computes the DC bias point of the circuit. All capacitors are open-circuited, all inductors are short-circuited. Reports node voltages and branch currents.

```spice
.OP
```

## .DC -- DC Sweep

```
.DC srcname start stop step [src2 start2 stop2 step2]
```

Sweeps a source value and computes the DC operating point at each step. Optionally nests a second sweep.

| Parameter | Description |
|-----------|-------------|
| `srcname` | Name of the source to sweep (e.g., `V1`) |
| `start`   | Starting value |
| `stop`    | Ending value |
| `step`    | Increment |

```spice
.DC V1 0 5 0.1
.DC V1 0 5 0.1 V2 0 3.3 1.1
```

## .AC -- AC Frequency Sweep

```
.AC DEC|OCT|LIN npts fstart fstop
```

Linearizes the circuit around its DC operating point and computes the small-signal frequency response.

| Parameter | Description |
|-----------|-------------|
| `DEC`     | Points per decade |
| `OCT`     | Points per octave |
| `LIN`     | Total points, linearly spaced |
| `npts`    | Number of points (per decade/octave, or total for LIN) |
| `fstart`  | Start frequency (Hz) |
| `fstop`   | Stop frequency (Hz) |

```spice
.AC DEC 10 1 1MEG
.AC LIN 100 60 60
```

## .TRAN -- Transient Analysis

```
.TRAN tstep tstop [tstart [tmax]] [UIC]
```

Time-domain simulation using numerical integration.

| Parameter | Description |
|-----------|-------------|
| `tstep`   | Suggested output time step |
| `tstop`   | End time |
| `tstart`  | Start saving data at this time (default: 0) |
| `tmax`    | Maximum internal time step (default: tstop/50) |
| `UIC`     | Use initial conditions -- skip DC operating point, use `ic=` values on devices |

```spice
.TRAN 1N 10U
.TRAN 10N 1M 0 100N UIC
```

## .SENS -- Sensitivity Analysis

```
.SENS V(node)
.SENS V(node1, node2)
.SENS I(source)
```

Computes the DC sensitivity of an output variable with respect to every circuit parameter.

```spice
.SENS V(out)
.SENS I(V1)
```

## .TF -- Transfer Function

```
.TF V(node[,ref]) input_source
.TF I(source) input_source
```

Computes the DC small-signal transfer function, input resistance, and output resistance.

```spice
.TF V(out) V1
.TF V(out, ref) V1
.TF I(Vload) V1
```

## .PZ -- Pole-Zero Analysis

```
.PZ node1 node2 node3 node4 VOL|CUR PZ|POL|ZER
```

Finds poles and zeros of a transfer function.

| Parameter | Description |
|-----------|-------------|
| `node1 node2` | Output port nodes |
| `node3 node4` | Input port nodes |
| `VOL`     | Voltage transfer function (V(node1,node2) / V(node3,node4)) |
| `CUR`     | Transimpedance (V(node1,node2) / I(input)) |
| `PZ`      | Find both poles and zeros |
| `POL`     | Find poles only |
| `ZER`     | Find zeros only |

```spice
.PZ out 0 in 0 VOL PZ
```

## Multiple analyses

A netlist can contain multiple analysis commands. They run sequentially:

```spice
RC Filter
V1 in 0 AC 1 DC 1
R1 in out 1K
C1 out 0 1U
.OP
.AC DEC 20 1 100K
.END
```

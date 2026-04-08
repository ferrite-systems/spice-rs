# Your First Simulation

What happens when you write `.OP` in a netlist and press Run? Here is the pipeline from text to numbers, with no math — just the flow.

---

## 1. Parse the netlist

The simulator reads your text file line by line. Each line describes a component (`R1 a b 1k`), a model (`.model NPN NPN`), or an analysis command (`.OP`, `.TRAN`). The parser turns these into an internal data structure — a list of devices with their connections and parameter values.

Parsing is unglamorous but critical. A single misread parameter propagates through the entire simulation. In spice-rs, the parser is validated against ngspice to ensure every model parameter reaches the device with the correct value.

---

## 2. Build the circuit

The simulator assigns a number to each node, creates a device object for each component, and allocates the MNA matrix. Each device registers which matrix positions it will use — this lets the sparse solver pre-allocate memory in the right pattern.

At this stage the matrix is empty. It has the right shape and sparsity structure, but all values are zero.

---

## 3. Stamp the matrix

Each device writes its contributions into the matrix and right-hand side vector. A resistor stamps its conductance, a voltage source stamps its constraint, a current source stamps the RHS. This is the `load()` function in spice-rs — every device has one.

For a linear circuit (resistors, fixed sources), stamping happens once. The matrix is filled with constant values.

---

## 4. Solve

The sparse solver takes the matrix $Gx = b$ and produces $x$ — the vector of node voltages and branch currents. It uses LU factorization, exploiting the fact that circuit matrices are overwhelmingly sparse (a 1000-node circuit might have only 5000 nonzero entries out of a million possible).

For a linear `.OP` analysis, this single solve is the answer.

---

## 5. Extract results

The solution vector gives node voltages directly. Branch currents for voltage sources are in the extra variables. Currents through other components are computed from the node voltages using the component equations.

The simulator prints the operating point table: the voltage at every node, the current through every voltage source.

---

## What about nonlinear circuits?

If the circuit contains diodes, transistors, or any nonlinear element, a single solve is not enough. The device equations depend on the solution — a MOSFET's conductance changes with $V_{gs}$ — so the simulator must iterate:

1. Guess an initial solution
2. Linearize each device at the current operating point
3. Stamp, solve, get a new solution
4. Check if it converged (did the solution change?)
5. If not, go back to step 2

This is Newton-Raphson iteration, covered in Chapter 3. The key point: the matrix is rebuilt and re-solved at every iteration, using the same stamp-and-solve machinery. The only difference is that the stamps change each time.

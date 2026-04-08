# Mutual Inductors

When two inductors are placed near each other, their magnetic fields interact. Current flowing through one inductor creates a magnetic flux that links through the other, inducing a voltage. This coupling is the basis of every transformer, and SPICE models it with the **K element** — a mutual inductor that specifies how strongly two inductors are coupled.

```text
L1 1 2 10m
L2 3 4 10m
K1 L1 L2 0.99
```

The K element doesn't exist as a physical device with its own terminals. It's a *modifier* that creates a coupling relationship between two existing inductors. The single parameter $k$ is the **coupling coefficient**, which ranges from 0 (no coupling) to 1 (perfect coupling):

- $k = 0$: The inductors are completely independent. No mutual interaction.
- $k = 1$: Perfect coupling. All magnetic flux from one inductor links through the other. This is the theoretical ideal for a transformer.
- $k = 0.99$: Tight coupling, typical of a well-designed transformer with a shared core.
- $k = 0.01$: Very loose coupling, typical of inductors that happen to be near each other on a PCB.

---

## Mutual inductance

The physical quantity that describes the coupling is the **mutual inductance** $M$:

$$M = k \sqrt{L_1 \cdot L_2}$$

$M$ has units of henries, just like self-inductance. It determines how much voltage is induced in one inductor by a changing current in the other:

$$V_1 = L_1 \frac{dI_1}{dt} + M \frac{dI_2}{dt}$$

$$V_2 = M \frac{dI_1}{dt} + L_2 \frac{dI_2}{dt}$$

Each inductor's voltage depends on its own current derivative (self-inductance) *plus* the other inductor's current derivative (mutual inductance). When $k = 0$, the mutual terms vanish and the inductors are independent. When $k = 1$ and $L_1 = L_2 = L$, the mutual inductance equals $L$ and the system is fully coupled.

In spice-rs, the mutual inductance factor is computed in [`src/device/mutual_inductor.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/device/mutual_inductor.rs), matching ngspice's `muttemp.c`:

```rust
// muttemp.c:56: MUTfactor = MUTcoupling * sqrt(fabs(ind1 * ind2))
self.factor = self.coupling * (self.ind1_value * self.ind2_value).abs().sqrt();
```

---

## How coupling enters the simulation

The mutual inductor modifies the simulation in two ways: it adds **cross-flux** during the pre-load phase and **cross-coupling stamps** during the load phase.

### Pre-load: adding mutual flux

Recall from the inductor chapter that each inductor's `pre_load()` computes its flux as $\Phi = L \cdot I$. After all inductors have set their self-flux, the mutual inductors add the cross-coupling contribution:

$$\Phi_1 \mathrel{+}= M \cdot I_2$$
$$\Phi_2 \mathrel{+}= M \cdot I_1$$

```rust
// pre_load(): add cross-coupling flux
let i1 = mna.rhs_old_val(self.ind1_branch);
let i2 = mna.rhs_old_val(self.ind2_branch);
let f1 = states.get(0, self.ind1_flux_offset);
states.set(0, self.ind1_flux_offset, f1 + self.factor * i2);
let f2 = states.get(0, self.ind2_flux_offset);
states.set(0, self.ind2_flux_offset, f2 + self.factor * i1);
```

When the inductors then run their `load()` function, they integrate the *total* flux (self + mutual) to compute their companion models. The mutual contribution automatically appears in the companion voltage source.

### Load: matrix cross-coupling

The mutual inductor also stamps cross-coupling terms into the MNA matrix. These terms link the branch equation of one inductor to the branch current of the other:

$$\text{G}[\text{branch}_1, \text{branch}_2] \mathrel{-}= M \cdot \alpha_0 / h$$
$$\text{G}[\text{branch}_2, \text{branch}_1] \mathrel{-}= M \cdot \alpha_0 / h$$

where $\alpha_0/h$ is the leading integration coefficient (`ag[0]`). This ensures that Newton-Raphson correctly accounts for the mutual coupling when computing the Jacobian.

```rust
// load(): stamp cross-coupling into matrix
let stamp_val = -(self.factor * self.ag[0]);
mna.stamp(self.ind1_branch, self.ind2_branch, stamp_val);
mna.stamp(self.ind2_branch, self.ind1_branch, stamp_val);
```

---

## AC behavior

In AC analysis, the mutual coupling adds off-diagonal imaginary terms between the two inductor branch equations:

$$\text{G}_{\text{imag}}[\text{branch}_1, \text{branch}_2] \mathrel{-}= \omega M$$
$$\text{G}_{\text{imag}}[\text{branch}_2, \text{branch}_1] \mathrel{-}= \omega M$$

Combined with each inductor's own $-j\omega L$ on its branch diagonal, the AC system captures the full frequency-dependent coupling between the two inductors.

---

## Transformers

The most common use of mutual inductors is to model transformers. An ideal transformer with turns ratio $N_1:N_2$ is approximated by:

$$L_1, \quad L_2 = L_1 \cdot \left(\frac{N_2}{N_1}\right)^2, \quad k \approx 0.99$$

The voltage transformation ratio follows from the inductance ratio:

$$\frac{V_2}{V_1} = \frac{N_2}{N_1} = \sqrt{\frac{L_2}{L_1}}$$

A practical transformer has $k$ slightly less than 1 — the difference $1 - k$ represents the leakage flux that doesn't couple between windings. This leakage shows up as a small inductance in series with each winding that limits the transformer's high-frequency response.

```text
Example: a 10:1 step-down transformer

L1 primary_p primary_n 100m
L2 secondary_p secondary_n 1m
K1 L1 L2 0.99
```

With $L_1 = 100$ mH and $L_2 = 1$ mH, $\sqrt{L_2/L_1} = 0.1$, so the turns ratio is 10:1. The mutual inductance is $M = 0.99\sqrt{0.1 \cdot 0.001} = 0.0099$ H.

---

## Load sequence

In spice-rs, the correct simulation of mutual inductors depends on the load sequence:

1. All inductors run `pre_load()` — each sets $\Phi = L \cdot I$
2. All mutual inductors run `pre_load()` — each adds $M \cdot I_{\text{other}}$ to both inductors' flux
3. All mutual inductors run `load()` — stamp cross-coupling into matrix
4. All inductors run `load()` — integrate total flux, stamp companion models

This ordering is enforced by the device type ordering system: inductors have `type_order 29`, mutual inductors have `type_order 30`. The pre-load pass runs in type order, ensuring inductors compute self-flux before mutual inductors add cross-flux. By the time inductors run their main `load()`, the flux state already contains both self and mutual contributions.

<!-- TODO: interactive transformer — two coupled inductors with adjustable k, L1, L2; drive one winding with a sine wave, see the voltage induced in the other; show how k affects coupling and leakage -->

# Breakpoints

Adaptive timestep control works beautifully when the waveform is smooth — the LTE estimator sees the curvature and adjusts accordingly. But what happens when a source has an *abrupt* transition? A PULSE source with a 1 ns rise time. A PWL source with a sharp corner. If the simulator is cruising along with a 10 us timestep and the pulse edge is at $t = 50\text{ us}$, it will step right over the edge from 40 us to 50.01 us and never see the transition at all.

The solution is **breakpoints**: a list of future times where the simulator knows something interesting will happen. The transient engine consults this list before each step and ensures that a timestep lands exactly on each breakpoint.

## How breakpoints work

Each source with piecewise-defined behavior registers its transition times with the breakpoint system. A PULSE source registers the rise start, rise end, fall start, and fall end times for each period. A PWL source registers every corner point. The breakpoint list is maintained in sorted order, and the transient engine checks it at every step.

The logic has three cases:

**1. At a breakpoint.** The simulator is currently sitting on a registered breakpoint. It knows a transition is happening right now. The response:
- Drop the integration order to 1 (backward Euler) — the solution may be discontinuous, and backward Euler handles discontinuities gracefully
- Restrict the timestep: $h \leq 0.1 \cdot \min(\text{save\_delta}, \text{next\_bp} - \text{this\_bp})$
- On the very first time after a breakpoint, cut $h$ by another factor of 10

This aggressive timestep reduction ensures the simulator takes many small steps through the transition, capturing the fast edge with high fidelity.

**2. About to overshoot a breakpoint.** The current time plus the proposed timestep would land past the next breakpoint. The response:
- Save the current timestep for later: `save_delta = delta`
- Clip the timestep to land exactly on the breakpoint: `delta = bp - time`

This is the critical case: without it, the simulator might skip over the breakpoint entirely.

**3. Far from a breakpoint.** The next breakpoint is well beyond the proposed step. No action needed — adaptive timestep control operates normally.

```text
    Breakpoint clipping

    time            proposed step           breakpoint
     │                  │                       │
     ●──────────────────┼───────────────────────●
     t                  t+delta                 bp

     The proposed step overshoots the breakpoint.
     Clip: delta = bp - t

     time    clipped step  breakpoint
     │          │              │
     ●──────────●──────────────●
     t         bp            (next bp)

     Now the simulator lands exactly on the breakpoint.
```

## The breakpoint list in spice-rs

The `Breakpoints` struct in [`breakpoint.rs`](https://github.com/nickvdl/spice-rs/blob/main/src/breakpoint.rs) maintains a sorted list of future breakpoint times with two invariants:

- Always at least 2 elements: `times[0]` is the next breakpoint, and the last element is the `final_time` sentinel
- Always sorted ascending

Sources register breakpoints via `set()`, which inserts in sorted order and merges breakpoints that are too close together:

```text
    Breakpoint merging (min_break = max_step * 5e-5)

    If two breakpoints are within min_break of each other,
    they are merged into one. This prevents the timestep
    from being forced impossibly small between two nearly
    simultaneous events.

    Example: min_break = 5e-10 for max_step = 10us

    ●──────────────────●●──────────────────●
                       ▲▲
                These two are within 5e-10
                → merged into one breakpoint
```

The `min_break` tolerance is $5 \times 10^{-5} \cdot \text{max\_step}$, following the ngspice convention. For a typical `max_step` of 10 us, this is 0.5 ns — well below the resolution of any practical signal, but large enough to prevent floating-point issues.

## Source breakpoint registration

Breakpoints are registered dynamically, not all at once. At each accepted timestep, `register_source_breakpoints()` asks each PULSE and PWL source: "given the current time, when is your next transition?" The source computes the answer based on its waveform parameters and the current position within the period.

This dynamic registration is important for efficiency: a PULSE source with a 1 MHz repetition rate and a 1 ms simulation would have 1000 periods. Pre-registering all 4000+ edges (rise start, rise end, fall start, fall end for each period) would be wasteful. Instead, the source registers only the next upcoming edge, and registers the following edge once the current one has passed.

From `transient()`:

```text
    At each accepted timestep:
        for each source:
            if time >= source.break_time:
                next_bp = source.next_breakpoint(time, ...)
                breakpoints.set(next_bp, time)
```

## The breakpoint-timestep dance

The interaction between breakpoints and timestep control is one of the most delicate parts of the transient engine. Here's the full sequence for a PULSE rise edge:

```text
    1. Approaching the edge:
       ●───────────────────────────●  (breakpoint at rise start)
       Adaptive timestep growing...
       Clip to hit breakpoint exactly.

    2. At the breakpoint:
       Order → 1, delta = 0.1 * min(save_delta, next_bp - bp)
       ●─●─●─●─●────●────●────●   (small steps through the rise)

    3. After the transition:
       LTE sees smooth waveform, promotes to order 2
       ●────●────────●──────────●  (timestep grows back)

    4. Approaching the next breakpoint (rise end or fall start):
       Clip again, repeat.
```

The `save_delta` mechanism remembers the timestep the engine was using *before* it clipped to hit the breakpoint. After passing through the transition, the engine uses this saved value as an upper bound on how quickly the timestep can recover, preventing it from jumping back to a large step too abruptly.

## What happens without breakpoints

Without breakpoints, the simulator relies entirely on the LTE mechanism to detect transitions. The LTE looks at the *local* behavior of the waveform — it estimates error from the curvature at the current position. It has no way to anticipate a future discontinuity.

The result: the simulator takes a large step that lands past the edge, the NR solver either fails to converge (causing a reject and timestep reduction) or converges to a solution that skips the edge detail. In the worst case, the simulation produces incorrect results without any warning — the fast edge is smoothed over, and the output looks plausible but wrong.

Breakpoints prevent this by giving the transient engine foresight. They are the mechanism by which the source waveform definition communicates with the integration engine.

<!-- TODO: interactive breakpoint visualization — show a PULSE waveform with breakpoints marked, step through time, show how the simulator clips and shrinks timesteps around edges -->

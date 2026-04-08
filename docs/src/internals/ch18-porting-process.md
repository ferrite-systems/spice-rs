# The Porting Process

spice-rs was built by systematically reading ngspice C source code and translating it to Rust. Not reimplementing, not approximating — translating. The distinction matters.

SPICE has 50 years of accumulated numerical tricks. Every `if`-statement, every magic constant, every seemingly redundant check exists because someone hit a real circuit that broke without it. The ngspice codebase encodes the hard-won solutions to thousands of convergence edge cases, numerical stability issues, and device physics subtleties. Trying to "improve" or "simplify" this code without understanding every line is how simulator ports fail.

This chapter documents the methodology that achieved 200/224 test circuits passing with 176 producing bit-identical results to ngspice.

## Chapters

- [Philosophy](ch18-01-philosophy.md) — "Port, don't approximate."
- [Investigation-First Method](ch18-02-investigation-first.md) — Read the C, document the algorithm, then translate
- [Eval Harness](ch18-03-eval-harness.md) — The spice-eval validation framework
- [Case Study: MOSFET Level 1](ch18-04-case-mosfet1.md) — Porting mos1load.c
- [Case Study: BSIM3v3](ch18-05-case-bsim3.md) — Porting 5000 lines of b3ld.c
- [Case Study: Markowitz Solver](ch18-06-case-markowitz.md) — Porting Sparse 1.3 to arena-based Rust

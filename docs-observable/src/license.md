---
title: Licensing & Attribution
toc: true
---

# Licensing & Attribution

spice-rs is a faithful port of [ngspice](https://ngspice.sourceforge.io/) to Rust. The algorithms, device model equations, and numerical methods in this project are derived from the ngspice source code and its upstream ancestors. This page documents the origins, licenses, and contributors of all upstream code.

---

## spice-rs

spice-rs is original Rust code, but the algorithms it implements are direct translations of C code from ngspice and SuiteSparse. Each section of spice-rs is licensed to match its upstream source:

- **BSD-3-Clause** — the majority of spice-rs (device models, analysis algorithms), derived from ngspice's BSD-licensed code. See `LICENSE-BSD`.
- **MIT** — sparse matrix code derived from Sparse 1.3 (Kundert). See `LICENSE-MIT`.
- **LGPL-2.1-or-later** — applies to the sparse-rs dependency (derived from SuiteSparse KLU/BTF) when distributed as a linked binary.

---

## ngspice

**Source:** [ngspice.sourceforge.io](https://ngspice.sourceforge.io/)
**License:** Modified BSD (3-Clause) for the majority of the codebase

ngspice is the open-source successor to Berkeley SPICE3f5. Its source code integrates contributions from Spice3f5, XSPICE, CIDER, numparam, tclspice, and many individual contributors over 40+ years of development.

### What spice-rs ports from ngspice

| spice-rs module | Derived from (ngspice) | License |
|---|---|---|
| Device models (MOS1–3, BSIM3/4, BJT, Diode, JFET, MESFET, etc.) | `src/spicelib/devices/` | Modified BSD |
| MNA matrix assembly | `src/spicelib/analysis/` | Modified BSD |
| Newton-Raphson convergence | `src/spicelib/analysis/` | Modified BSD |
| Transient integration (trapezoidal, Gear) | `src/spicelib/analysis/` | Modified BSD |
| AC & DC analysis | `src/spicelib/analysis/` | Modified BSD |
| Sparse matrix solver (Markowitz) | `src/maths/sparse/` | MIT |

### ngspice BSD License

```
Copyright 1985-2018, Regents of the University of California and others

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
   notice, this list of conditions and the following disclaimer in the
   documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### Other licenses in ngspice (not ported)

The following ngspice subsystems use licenses other than Modified BSD. spice-rs does **not** incorporate code from these sections, but they are documented here for completeness:

| ngspice directory | License | Notes |
|---|---|---|
| `src/maths/KLU` | LGPLv2.1 | KLU sparse solver (spice-rs uses sparse-rs instead) |
| `src/frontend/numparam` | LGPLv2+ | Parameter expansion |
| `src/tclspice.c` | LGPLv2 | Tcl interface |
| `src/osdi` | MPLv2.0 | Open Source Device Interface |
| `src/xspice` | Public Domain | Code modeling (Georgia Tech) |
| `src/xspice/icm/table` | GPLv2+ | Table models |
| `src/spicelib/devices/ndev` | Public Domain | Numerical device models |

---

## Berkeley SPICE3f5

ngspice descends from Berkeley SPICE3f5, the foundational circuit simulator developed at UC Berkeley over 30+ years. The core algorithms for Modified Nodal Analysis, Newton-Raphson iteration, and the original device model equations (Shichman-Hodges MOSFET, Gummel-Poon BJT) originate from this work.

**License:** BSD (UC Berkeley)
**Authors:** Thomas L. Quarles, A. Richard Newton, Donald O. Pederson, Andrei Vladimirescu, and others at UC Berkeley EECS

---

## SuiteSparse (via sparse-rs)

spice-rs depends on [sparse-rs](./internals/ch19), a separate crate that ports sparse matrix algorithms from [SuiteSparse](https://people.engr.tamu.edu/davis/suitesparse.html). sparse-rs is licensed under LGPL-2.1-or-later.

| Component | Copyright | License |
|---|---|---|
| AMD | (c) 1996-2023 Timothy A. Davis, Patrick R. Amestoy, Iain S. Duff | BSD-3-Clause |
| COLAMD | (c) 1998-2023 Timothy A. Davis, Stefan Larimore | BSD-3-Clause |
| BTF | (c) 2004-2023 University of Florida, Timothy A. Davis, Ekanathan Palamadai | LGPL-2.1-or-later |
| KLU | (c) 2004-2023 University of Florida, Timothy A. Davis, Ekanathan Palamadai | LGPL-2.1-or-later |

---

## Sparse 1.3 (Kundert)

The Markowitz pivot search and sparse factorization algorithms in both ngspice and sparse-rs descend from the Sparse 1.3 package.

**License:** MIT (unnamed, compatible with New BSD)
**Author:** Kenneth S. Kundert, UC Berkeley (advisor: Alberto Sangiovanni-Vincentelli)

---

## License summary

| Component | License | Obligation |
|---|---|---|
| **spice-rs** (most code) | BSD-3-Clause | Attribution in source and binary distributions |
| **Sparse matrix code** | MIT | Attribution in source distributions |
| **sparse-rs** (dependency) | LGPL-2.1-or-later | sparse-rs source must be available when distributing linked binaries |

---

## ngspice contributors

Spice was originally written at The University of California at Berkeley. The following people have contributed to ngspice, the open-source project from which spice-rs derives its algorithms:

Vera Albrecht, Cecil Aswell, Giles Atkinson, Giles C. Billingsley, Phil Barker, Steven Borley, Krzysztof Blaszkowski, Stuart Brorson, Arpad Burmen, Alessio Cacciatori, Mansun Chan, Wayne A. Christopher, Al Davis, Glao S. Dezai, Jon Engelbert, Daniele Foci, Henrik Forsten, Noah Friedman, David A. Gates, Alan Gillespie, John Heidemann, Marcel Hendrix, Jeffrey M. Hsu, JianHui Huang, S. Hwang, Chris Inbody, Gordon M. Jacobs, Min-Chie Jeng, Beorn Johnson, Stefan Jones, Kenneth H. Keller, Vadim Kuznetsov, Francesco Lannutti, Robert Larice, Mathew Lew, Robert Lindsell, Weidong Liu, Kartikeya Mayaram, Richard D. McRoberts, Manfred Metzger, Jim Monte, Wolfgang Muees, Paolo Nenzi, Gary W. Ng, Hong June Park, Arno Peters, Stefano Perticaroli, Serban-Mihai Popescu, Georg Post, Thomas L. Quarles, Emmanuel Rouat, Jean-Marc Routure, Jaijeet S. Roychowdhury, Lionel Sainte Cluque, Takayasu Sakurai, Carsten Schoenert, AMAKAWA Shuhei, Kanwar Jit Singh, Bill Swartz, Hitoshi Tanaka, Brian Taylor, Steve Tell, Linus Torvalds, Andrew Tuckey, Robert Turnbull, Andreas Unger, Holger Vogt, Dietmar Warning, Michael Widlok, Charles D.H. Williams, Antony Wilson, Pascal Kuthe, and many others.

*If someone has been omitted from this list, the omission was unintentional. The canonical contributor list is maintained in the [ngspice AUTHORS file](https://sourceforge.net/p/ngspice/ngspice/ci/master/tree/AUTHORS).*

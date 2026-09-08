# Coherent response constructions and resource prices

The response audit distinguishes three statements with different logical
directions:

1. A constructed circuit gives an **upper** bound on the cheapest circuit.
2. A circuit lower bound must apply to **every** circuit in its stated model.
3. A declared resource price is a **premise** of the Category 1 corollary.

An expensive construction does not prove a minimum price. Adding omitted
operations to a construction cannot, by itself, raise the security claim.
The [Category 1 derivation](category-one.md#coherent-response-cost) retains
its declared prices and states their batch-amortization requirement explicitly.

## Scope and provenance

The audit addresses the missing-reduction observation in Borealis's
[submission #9](https://git.parano1d.org/ignotusnemo/parano1d-soundness/pulls/9)
and the scalar target-touch argument in Delta's
[submission #11](https://git.parano1d.org/ignotusnemo/parano1d-soundness/pulls/11).
The latter is proved here for exact unitary circuits; no extension to
measurement, classical feed-forward or free output relabeling is asserted.

The protocol instance is imported directly from the production
[`noid_poseidon2b`](../../noid_poseidon2b/src/native/permutation.rs) crate. It
is the width-four, 128-bit-field Poseidon2b permutation with eight full rounds,
58 partial rounds, 90 applications of the seventh-power S-box and 67 matrix
layers including the initial layer. `ProductionParameters::load` also imports
the exact external and internal matrices used by the native permutation, so
the audit and its correspondence tests track the production configuration.

The logical gate basis is CNOT, one-qubit Clifford and T, with all-to-all
connectivity and initially zero ancillas. Gates in one layer have disjoint
wire sets. A Toffoli is charged as its exact decomposition into six CNOT,
two one-qubit Clifford and seven T gates, with depth at most eight. Wire
routing on restricted hardware, error correction and physical runtime are
outside this logical circuit model.

The decomposition and unreduced Karatsuba schedule are from Jang et al.,
[*Quantum Binary Field Multiplication with Optimized Toffoli Depth and
Extension to Quantum Inversion*](https://doi.org/10.3390/s23063156),
Sensors 23(6), 3156 (2023), Sections 2.3 and 3.1–3.2. Table 1 explicitly
excludes modular reduction; Section 3.5 discusses that separate operation.
The reviewed PDF has SHA-256
`e8b31e10131585e72369cab6297a1d27224afe45e01ce352d7c622bdabe6b7e0`.

## Polynomial multiplication is not field multiplication

For two 128-coefficient polynomials, the recursive schedule has

\[
C_{\rm structural}
=\sum_{i=0}^{6}3^i\bigl(5(128/2^i)-4\bigr)=16\,218,
\qquad T_{\rm Toffoli}=3^7=2\,187.
\]

After decomposing the Toffolis, the gate inventory is

\[
29\,340\text{ CNOT}+4\,374\text{ one-qubit Clifford}
+15\,309\text{ T}=49\,023.
\]

This produces a polynomial of degree at most 254. It is not yet the
production field product modulo

\[
f(X)=X^{128}+X^7+X^2+X+1.
\]

For example, the unreduced low lane of \(X\cdot X^{127}\) is zero,
whereas its field product is \(X^7+X^2+X+1\), represented by `0x87`.
This is a concrete regression test, not just a resource-label correction.

### Constructive polynomial invariant

The generator in [reversible_multiplier.rs](../src/reversible_multiplier.rs)
preserves its two input registers. At size \(n=2m\), it computes

\[
A=a_0b_0,\quad C=a_1b_1,\quad
B=(a_0+a_1)(b_0+b_1).
\]

Preparing the two sums uses \(2n\) CNOTs. The result is
\(A+X^m(A+B+C)+X^{2m}C\). The generator first adds \(A,C\) to the
middle register, then merges the overlapping high coefficients of \(A\)
and low coefficients of \(C\). These operations use \(3n-4\) CNOTs.
At \(n=1\), a fresh zero wire is targeted by a Toffoli.
Induction proves the polynomial result and the recurrence above.

The paper's depth formula gives 43. The generated disjoint-wire schedule
has depth 42: the size-two recursion needs no third merge layer. A Toffoli
occupies all its wires for eight layers in this scheduler, so replacing
each by the cited exact decomposition fits within the reported depth.

### Reduction, copy and cleanup

For each coefficient \(k=254,253,\ldots,128\), add its current bit to
coefficients \(k-128+\{0,1,2,7\}\), using four CNOTs. Descending order
also reduces any newly produced high coefficients. This costs exactly
\(127\cdot4=508\) CNOTs. High wires remain as workspace; they are not
irreversibly erased.

The complete forward field computation has 49,531 gates and depth 49.
Copy its 128 low output wires into an arbitrary response register using
128 CNOTs, then reverse the **entire** forward circuit. The result is

\[
|a,b,z,0\rangle\longmapsto|a,b,z\mathbin{\mathsf{XOR}}ab,0\rangle.
\]

Its gate count is \(2(49\,023+508)+128=99\,190\), its scheduled depth
is 99, and it uses 6,689 wires including the input and response registers.
The forward computation needs 6,305 additional wires beyond its two
128-bit inputs; its output is already among these additional wires.

All operations are exact reversible basis permutations. Correctness on
basis states therefore extends to arbitrary coherent superpositions without
input-dependent phases.

## Complete scalar Poseidon2b construction

The following upper bound composes explicit finite circuit generators. It
is deliberately conservative, not an optimized implementation or a claim
that a 100-million-gate network was instantiated and simulated in full.
In particular it includes linear operations, fanout, representation changes,
output copy and cleanup rather than only nonlinear multiplication.

### Input-preserving binary linear maps

For any \(n\times n\) binary matrix with \(n=2^r\), compute its
linear map while retaining every original input wire:

1. For each input bit, create \(n\) dedicated copies with CNOTs. The
   first copy uses the original wire; a doubling tree supplies the rest.
   All inputs together require \(n^2\) gates and depth at most \(r+1\).
2. Assign one copy of each input to each output parity. Compute each
   required parity by a balanced CNOT tree and copy it to a fresh output
   wire. The parities have disjoint workspaces. This costs at most
   \(n^2\) further gates and depth at most \(r+1\).

Thus the map uses at most

\[
g_L(n)=2n^2,\qquad d_L(n)=2\log_2 n+2,
\qquad w_L(n)=n^2+n
\]

additional wires, including its outputs. Zero rows require no parity
gates. CNOT fanout here is reversible entangling fanout on computational
basis bits, not cloning arbitrary quantum states. None of the original
inputs becomes a parity-tree target. This preservation matters when both
\(x\) and \(x^2\) are reused by an S-box. The whole map, including
its retained workspace, is reversed during global cleanup.

Squaring, tower/flat basis changes and the actual multiplication by either
production MDS matrix are binary linear maps. The bound therefore applies
to their exact coefficients, without assuming they are free.

### S-box and round composition

Use the native addition chain

\[
x_2=x^2,\quad x_4=x_2^2,\quad x_3=x x_2,\quad x_7=x_3x_4.
\]

Two input-preserving linear squarings and two forward field multipliers,
scheduled serially as a conservative bound, give one S-box

\[
g_S=2g_L(128)+2(49\,531)=164\,598,
\qquad d_S=2d_L(128)+2(49)=130,
\]

with \(w_S=2w_L(128)+2(6\,305)=45\,634\) additional wires.
The four S-boxes in a full round act on disjoint workspaces and run in
parallel. A partial round applies one S-box. Both have depth at most 130.

For a 512-bit state, each matrix layer uses at most 524,288 gates, depth
20 and 262,656 additional wires. Public round constants use at most 128
X gates per active S-box lane, at most one parallel layer per round.
There are four tower-to-flat input conversions and one flat-to-tower
conversion for the requested scalar output. The four input conversions
run in parallel, so conversion contributes two sequential depth stages.

The complete forward computation has upper bounds

\[
\begin{aligned}
G_f&=90g_S+67g_L(512)+90\cdot128+5g_L(128)
     =50\,116\,476,\\
D_f&=2d_L(128)+d_L(512)+66(1+d_S+d_L(512))
     =10\,018.
\end{aligned}
\]

Copy the scalar result into the caller's 128-bit response register and
reverse this complete forward computation. For
\(f(s)=\pi_0(P(s))\), this implements

\[
U_f:|s,z,0\rangle\longmapsto
|s,z\mathbin{\mathsf{XOR}}f(s),0\rangle
\]

with the simultaneous upper bounds

\[
\begin{aligned}
G_U&=2G_f+128=100\,233\,080,\\
D_U&=2D_f+1=20\,037,\\
G_UD_U&=2\,008\,370\,223\,960,\\
W_U&=512+128+90w_S+67w_L(512)+5w_L(128)
     =21\,788\,212.
\end{aligned}
\]

The width is intentionally large. No gate-depth improvement over all
possible constructions is claimed. The bound is for one scalar response,
not for an optimized wallet or History duplex-query vector.

## Independent scalar lower bound

For the same fixed-register unitary model, the audit proves
\(G\ge128\), \(D\ge1\), hence \(GD\ge128\).
These are circuit counts, **not 128 security bits**.

First, \(\gcd(7,2^{128}-1)=1\), so \(x\mapsto x^7\) permutes the
field. Independent Leibniz determinant evaluation in the tower field gives
`0x40` for the external matrix and `0x2064` for the internal matrix.
Both are nonzero. Matrix layers, constants and S-box layers are therefore
bijective, and so is their composition \(P\).

It follows that \(\pi_0(P(s))\) takes every 128-bit value. For each named
response bit \(z_i\), there is an input requiring it to flip. If no gate
ever targets that bit, every CNOT using it only as a control commutes with
its computational-basis observable, and all other gates act elsewhere.
Its value cannot flip. Thus every one of the 128 response wires must be
targeted at least once. Each gate in the stated basis has at most one such
target, proving the gate bound. A nonidentity response needs depth at least
one. Allowing retained garbage does not change this argument.

This proof does not establish the much larger declared Category 1 price,
the total scalar gate charge, or amortized vector-query prices.

## Executable checks and interpretation

Run `cargo test --release --locked -p noid_soundness` and
`cargo run --release --locked -p noid_soundness -- --exact` from the repository
root.

The tests check the reduction on all 255 coefficient basis vectors and
its exact inverse; the field response on all \(128^2\) bilinear basis
pairs and additional dense inputs; all GF(16) input/input/response triples;
restoration of every workspace wire; and input-preserving linear networks
on all eight-bit inputs for several matrices. Bilinearity of the generated
field product together with the complete basis-pair check proves its
arithmetic correctness for all field inputs. Separate tests pin the
simultaneous scalar construction bounds and reject singular linear layers
for the surjectivity proof.

These checks correct the circuit accounting. They do not turn the
construction upper bound into a cryptographic lower bound, automatically
approve a research submission, or modify a frozen historical evidence
record. The current resource calculation uses the separate batch-cost
lemma in [category-one.md](category-one.md#typed-parallel-qrom-resource-theorem).

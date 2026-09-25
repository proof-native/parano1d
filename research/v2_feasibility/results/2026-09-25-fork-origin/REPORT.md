# Authenticated fork origin without legacy matrix rows

The [joint integer-core candidate](../2026-09-24-banked-integer-core/REPORT.md)
now has an independently verified retirement certificate for its legacy
boundary at height 9. This is the isolated height-10 candidate, not a mainnet
release artifact. Only legacy B25 was live in this fixture; a boundary with
both legacy classes still needs separate qualification.

The certificate replays the actual old terminal, reduces its outstanding
matrix claims and proves their evaluation under independently pinned
preprocessing keys. The live B25 key was recomputed from authenticated old
rows and matched its release pin. The inactive B255 key was reused only
under its independent release pin. A peer cannot choose those keys.

The certificate is **7,355,997 bytes**. Preparation on an i7-1365U with
12 Rayon threads took **453.05 seconds**, with peak RSS **7,135,896 KiB**
and no swap under a 20 GiB limit. This is offline certificate preparation,
not a recurring block-production cost. Its purpose is independent verification
without old rows; it is not a claim that certificates are always smaller than
compressed matrices.

A fresh verifier whose legacy matrix source unconditionally fails accepted
the certificate. With affinity to CPUs `0,2,4,6`, four Rayon threads, the PCLMUL
backend and an 8 GiB memory limit, certificate verification took **801 ms**.
The whole process, including metadata/key initialization, took **6.55 s**
and peaked at **86,468 KiB**. This is one laptop sample under constrained
resources, not a server latency distribution.

A second process verified the existing v2 terminal at height 37 with that
certificate and the two new matrices. It then exercised the node's protocol
component: reject an absent certificate, authenticate and retain it, recreate
the origin cache, authenticate the saved certificate again, and verify the
terminal. Old-format origin transport failed without old rows, and a tampered
terminal failed even after the origin was cached. This is a component restart
check, not a full daemon restart or a cold synchronization test.

The legacy and retired paths produce exactly the same origin binding:
`1b526c5af7edf1b349a0708b1ad18088f99e7a90a0069e8f593bd1be4c7b852e`.
The successor bank remains
`e1a79797415bb6d8da4935566ed614a9ec616ddee3315686f75eb8090495a0cd`.
No existing v2 terminal was regenerated for the retired path.

Raw results and executable/source hashes are in
[preparation.json](preparation.json), [verification.json](verification.json)
and [source.json](source.json). Measurements ran sequentially without concurrent
builds or tests. The full v2 verification process included initial semantic
authentication of file-loaded new matrices; that setup is not steady block
verification latency. Final soundness accounting, both old classes at the
boundary, executable integration and whole-node qualification remain required.

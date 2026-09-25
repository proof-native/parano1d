# Both legacy classes at the v2 boundary

The current joint integer-core bank passed a transition whose last old block
uses B255 and whose accumulated history contains both old classes. The input
is the authenticated isolated chain through H9; only those old blocks were
reused. Eight fresh v2 blocks were produced under the current candidate bank
at H10–17, including six templates, fourteen calls, due-height checks and a
durable close/reopen of the producer and receiver State.

The new sequence took 398.44 seconds on the i7-1365U. Initial authentication
of file-loaded canonical matrices is included in that duration; it is not a
steady embedded-node verification measurement. The initial H10 preparation
also includes new-matrix setup. Later empty and mixed preparations were
15.47–21.70 seconds in this component run. Full-node measurements remain in
the separate [capacity report](../2026-09-25-live-full-capacity/REPORT.md).

## Retirement evidence

A fresh certificate binds this exact boundary and the current bank. Both
preprocessing keys were recomputed from authenticated published legacy rows
and matched their independent release pins. The two sparse proofs are
6,478,232 and 7,628,536 bytes; the complete certificate is **15,093,850 bytes**.

Offline preparation took **2,527.97 seconds** with twelve Rayon threads,
peak RSS **20,926,196 KiB**, a 20 GiB cgroup limit and no swap. The disk-backed
workspace caused reclamation near that limit. The command's 64 GiB planning
allowance is an admission bound, not measured resident memory. This cost
belongs to preparation for one fork origin, rather than each new block.

With four logical CPUs, PCLMUL and an 8 GiB limit, independent certificate
verification took **1,429 ms**. The verifier's legacy matrix source always
fails. It then verified the existing H17 v2 terminal using the two new
matrices and exercised the protocol component's lifecycle:

- reject a missing certificate;
- authenticate, install and retain the certificate;
- reject old-format evidence when old rows are unavailable;
- recreate the context and authenticate the saved certificate again;
- reject a changed terminal even after the origin is cached.

That complete process took 207.53 seconds and peaked at 568,980 KiB. Of this,
187.92 seconds covered H17 verification including initial semantic
authentication of file-loaded new matrices. It must not be reported as a
recurring block-verification latency. The certificate-only time above is
measured separately inside the same process.

Origin binding:
`9eb210fa9c17e820e1aef15eb7b1975d7f5ec0fb5bc1aebeefa7217731344268`.
Candidate bank:
`e1a79797415bb6d8da4935566ed614a9ec616ddee3315686f75eb8090495a0cd`.
This is the isolated H10 candidate; it is not a source-scheduled mainnet pack.

Exact records are in [production.json](production.json),
[preparation.json](preparation.json), [verification.json](verification.json)
and [source.json](source.json). Executable SHA-256 digests identify the tested
binaries; the captured source HEAD identifies the measurement context.
Jobs ran sequentially without concurrent builds or other proof tests.

These results cover the protocol component. A full daemon test of cold
synchronization followed by retired-node block production is tracked
separately; this report does not claim that result before it completes.

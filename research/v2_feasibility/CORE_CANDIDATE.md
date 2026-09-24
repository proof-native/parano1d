# Contract core for the next capacity measurements

The candidate preserves ordinary payments and uses permissionless objects in
the existing state tree. An object commits to its program, current state,
claim and recovery authorities, deadline height, recipients and spending rules.
Creation is an ordinary payment to that commitment. Each call consumes one
object and either retains its successor or closes it to the committed recipient.
Authorization uses the existing witness-hiding wallet capsule with the
deadline-selected authority; the block relation must prove the opening and
effects as well.

The current candidate has eight instructions per program and sixteen contract
calls per block. These limits are inputs to the measurements, not a final ABI.
Every instruction has an opcode and one field immediate. Arithmetic is over
the existing binary field; it is not integer balance arithmetic.

| Opcode | Operation |
|---|---|
| 0 | Keep state |
| 1 | Add immediate to state |
| 2 | Multiply state by immediate |
| 3 | Add `context × (state + immediate)` to state |
| 4 | Require context to equal immediate |
| 5 | Require state to equal immediate |
| 6 | Load context into state |
| 7 | Load immediate into state |

The fixed contexts, in instruction order, are the two epoch-anchor fields,
fee, successor amount, payment amount, the two payment-recipient fields and
the validity bitmap. No caller-supplied context is trusted independently of
the transaction body.

Separate integer constraints enforce balance, a fee ceiling, a continuing
reserve, a per-call payment ceiling and allowed continue/close paths before
and after the deadline. A per-call allowance is not a daily rate limit.
Refundable payments, time-locked vaults and allowance wallets use these same
operations and policies; they have no special deployment registry.

The measurement relation must contain these authorization, policy and
execution constraints even for empty blocks. Adding an opcode or changing
contexts or commitment encoding later can change the matrix. Test both
honest execution and malformed openings, authority substitutions, deadline
boundaries and forbidden state effects before freezing any artifact.

The existing v1 and v1.1 proof relation must remain distinct and retain its
published matrix identities. A height schedule alone does not authenticate
the first v2 proof: it must bind to the verified last legacy block and all
its outstanding matrix claims. The origin interface must allow a later
authenticated certificate without making old matrices permanent inputs to
every future release.

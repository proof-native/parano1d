# Network economics

Parano1d ties issuance to Live State capacity and prices persistent State
growth separately from ordinary transaction work.

## Unit

```text
1 NOID = 1,000,000 μNOID
```

NOID is the currency ticker; the wallet uses ① as its interface symbol.
All consensus amounts are integers in μNOID.

## Block reward

The starting subsidy is 50 NOID. It halves whenever the State domain expands
and never falls below 1 NOID:

| `log_slots` | Capacity | Block reward |
|---:|---:|---:|
| 24 | 16,777,216 slots | 50.000000 NOID |
| 25 | 33,554,432 slots | 25.000000 NOID |
| 26 | 67,108,864 slots | 12.500000 NOID |
| 27 | 134,217,728 slots | 6.250000 NOID |
| 28 | 268,435,456 slots | 3.125000 NOID |
| 29 | 536,870,912 slots | 1.562500 NOID |
| 30–32 | Up to 4,294,967,296 slots | 1.000000 NOID |

Expansion requires sustained 75% occupancy in a hard-finalized window. The
network therefore moves to a lower inflation tier only after materially using
the current State capacity.

## Launch development allocation

For the first three target-time years, each block subsidy is divided:

- 90% to the miner;
- 5% to the O(1) Network Fund;
- 5% to Parano1d Lab.

There is no premine. After height 4,730,400, the complete block subsidy goes to
the miner.

To avoid creating two extra live UTXOs in every block, the two development
shares are paid in one mandatory two-output system record every 4,320 target
blocks. The amount uses the reward tier active at that payout boundary. If the
State expands during the interval, the resulting difference remains unissued.

At the initial 50 NOID subsidy, the miner receives 45 NOID plus claimable fees,
and each fund receives 10,800 NOID per 4,320-block payout interval. There are
1,095 scheduled payouts through height 4,730,400 inclusive. The schedule is
height-based, not a wall-clock payment guarantee.

The payout schedule, recipients and amounts are derived statelessly from height
and `log_slots` and are proved inside `HistoryStep`. A miner cannot omit,
redirect or defer a due payout.

## Fees

The minimum transaction fee consists of:

- 5,000 μNOID per logical transaction;
- 100 μNOID per live input;
- 700 μNOID per live output;
- 2,500 μNOID times occupancy pressure for each net-new live slot.

Pressure multipliers are:

| Parent-State occupancy | Multiplier |
|---:|---:|
| Below 50% | 1× |
| 50% to below 75% | 2× |
| 75% to below 90% | 4× |
| 90% and above | 8× |

The State-growth component is burned. Base, input, output and voluntary tip
components are claimable by the miner.

A consolidation that turns several inputs into one output shrinks State and
pays no growth burn.

## Dynamic relay floor

Consensus fees and local relay policy are separate. The local dynamic floor
uses the greater of 5,000 μNOID and 90% of the median fee among the last 50
transactions admitted to that node's mempool, not a network-wide fee estimate.

A raised floor applies only under pressure.
Reaching 80% of either the transaction-count limit or retained-byte limit
enables it. Falling strictly below 50% of both limits, or emptying the mempool,
resets it to the default. Between those thresholds the current pressure state
is retained. The existing limits remain 1,024 transactions and 384 MiB.

A local relay-floor increase does not by itself evict already admitted
transactions. A higher consensus fee caused by parent-State occupancy is
different: normal block cleanup removes transactions that no longer meet that
minimum and releases their local reservations.

Wallet Auto fees use the actual transaction shape, current parent-State
occupancy and the local relay floor. A local fee rejection before admission
can trigger an Auto replan within the existing three-attempt submission
budget. Manual fees and already confirmed consolidation quotes are not
silently increased. There is no extra peer-fee polling, and Auto does not
promise confirmation under every other node's policy.

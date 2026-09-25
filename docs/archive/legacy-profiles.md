# v1 and v1.1 profiles

**Historical: applies before mainnet v2 H210537.** The original v1 → v1.1
activation remains H95125. The v2 fork does not rewrite those historical rules.

| Parameter | Historical value |
| --- | --- |
| v1 → v1.1 | H95125 |
| Target interval | 20 s |
| ASERT | 6 blocks / 120 s |
| B25 | m22 / 25 pages |
| B255 | m24 / 255 pages |
| Live-input maximum | 1,020 |
| Subsidy | max(50 / 2^(log_slots − 24), 1) NOID |
| Terminal cap, v1 | 1 MiB |
| Terminal cap, v1.1 shared paths | 1,100,000 bytes |

The page count excludes the primary coinbase. An additional mandatory system
record consumes one effective page. B25 was the default miner class. The first
completed B25 preparation permitted B255 when a four-times timing estimate fit
the 20-second target; eligible transaction pages then determined selection.
This automatic permission rule does not apply to v2.

The gross reward at State levels 24–29 was 50, 25, 12.5, 6.25, 3.125 and
1.5625 NOID, with a 1 NOID floor at levels 30–32. A level increase required
sustained 75% occupancy. The original allocation horizon was 4,730,400 blocks
and its nominal daily interval 4,320 blocks. The v2 transition preserves the
three-year target-time horizon from genesis using the changed interval; see
[network economics](../protocol/economics.md).

Historical per-block maxima were 256 fixed bodies, 1,020 live user inputs,
510 outputs and 1,530 actions, with at most 256 touched segments. The codec
still needs to decode these historical records. A codec maximum is not the
current v2 admission budget.

The legacy C1 accounting reported a 128-bit FRI target, 127-bit provable and
conjectured Block–Tiwari classical FS-FRI values, an ideal-QROM half-success
boundary of 64.707407428576 bits and a dominant Category 1 gate-depth floor of
173.391078499301 bits. These are different security quantities, not
interchangeable bit-strength labels. Current v2 ancestry accounting is given
in the [security model](../protocol/security-model.md).

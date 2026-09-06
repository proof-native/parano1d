#!/usr/bin/env python3
"""Check bounded missing-only recovery on a loopback-only H5 fixture chain.

The run directory must contain copies of the earlier isolated H72 producer
and funded capacity wallet as provider/ and wallet/. No live mainnet data,
public peers, mining difficulty, admission rules or payload limits are used
as test shortcuts. Fresh clients synchronize through ordinary P2P.
"""

import argparse
import collections
import json
import os
import re
import time
from pathlib import Path

import live_two_miner_fork_reorg_scenario as live


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--new-bin", type=Path, required=True)
    parser.add_argument("--legacy-bin", type=Path, required=True)
    parser.add_argument("--transactions", type=int, choices=(129, 256), default=256)
    parser.add_argument("--recovery-only", action="store_true",
                        help="stop after cold recovery and restart, without mining")
    args = parser.parse_args()
    base = args.run_dir.resolve()
    interfaces = [line.split(":")[0].strip()
                  for line in Path("/proc/net/dev").read_text().splitlines()[2:]]
    live.require(interfaces == ["lo"], "a loopback-only network namespace is mandatory")
    live.require(not (base / "result.json").exists(), "refusing to overwrite a run")
    live.require((base / "provider/data").is_dir() and (base / "wallet/data").is_dir(),
                 "missing copied isolated fixture directories")
    live.require(args.recovery_only or args.transactions == 256,
                 "the full-block/spillover scenario requires 256 transactions")
    live.BASE = base
    report = {"status": "running", "network_interfaces": interfaces,
              "new_binary_sha256": live.sha256(args.new_bin),
              "legacy_binary_sha256": live.sha256(args.legacy_bin),
              "transactions": args.transactions, "recovery_only": args.recovery_only,
              "checkpoints": {}, "logs": []}
    nodes = []
    sequence = 0

    def save():
        (base / "result.json").write_text(json.dumps(report, indent=2) + "\n")

    def record(name, value):
        report["checkpoints"][name] = value
        save()
        print(f"[checkpoint] {name}", flush=True)

    class Node(live.Node):
        def __init__(self, name, index, binary):
            super().__init__(name, 29200 + index * 10, 29201 + index * 10)
            self.binary = binary
            self.peak_rss_kib = 0
            nodes.append(self)

        def spawn(self, label, **kwargs):
            nonlocal sequence
            sequence += 1
            label = f"{sequence:02d}-{label}"
            previous = live.NODE_BIN
            live.NODE_BIN = self.binary
            try:
                started = super().spawn(label, **kwargs)
                report["logs"].append(str(self.log_path))
                return started
            finally:
                live.NODE_BIN = previous

        def sample(self):
            if self.proc is not None and self.proc.poll() is None:
                status = Path(f"/proc/{self.proc.pid}/status").read_text()
                rss = re.search(r"^VmHWM:\s+(\d+)", status, re.M)
                if rss:
                    self.peak_rss_kib = max(self.peak_rss_kib, int(rss.group(1)))

        def stop(self):
            self.sample()
            super().stop()

    def pool_ids(node):
        return {tx["tx_hash"] for tx in live.rpc(node.rpc_port, "getMempoolInfo")["txs"]}

    def converge(source, destination, height):
        live.wait_value(f"{destination.name} synchronizes h{height}",
                        lambda: live.exact_tip(source, destination), 600, interval=1)
        live.require(destination.height() == height, "wrong synchronized height")
        live.require(live.exact_headers(source, destination, height), "headers disagree")
        live.require(live.rpc(source.rpc_port, "getActiveSlotCount") ==
                     live.rpc(destination.rpc_port, "getActiveSlotCount"), "State counts disagree")

    def wait_pool(node, expected, label):
        started = time.monotonic()
        curve = []
        deadline = started + 660
        while time.monotonic() < deadline:
            node.sample()
            stats = live.rpc(node.rpc_port, "getMempoolStats")
            count = live.rpc(node.rpc_port, "getMempoolSize")
            if not curve or count != curve[-1]["size"]:
                curve.append({"seconds": round(time.monotonic() - started, 3),
                              "size": count, "stats": stats})
            if count == len(expected):
                live.require(pool_ids(node) == expected, "count matches but transaction IDs differ")
                record(label, {"seconds": time.monotonic() - started, "curve": curve,
                               "count": count, "exact_ids": True})
                return
            time.sleep(1)
        record(label, {"curve": curve, "timeout": True})
        raise live.LiveForkReorgError(f"{label} did not recover every transaction")

    save()
    try:
        provider = Node("provider", 0, args.new_bin)
        wallet = Node("wallet", 1, args.legacy_bin)
        provider.start("upgraded-provider")
        height = provider.height()
        live.require(height == 72, "unexpected copied producer checkpoint")
        wallet.start("legacy-wallet-with-real-proofs", seeds=[provider.seed])
        converge(provider, wallet, height)
        live.require(pool_ids(wallet) == set() and pool_ids(provider) == set(), "fixture is not empty")
        record("initial", {"height": height,
                           "balance": live.rpc(wallet.rpc_port, "walletGetBalance")})
        address = live.rpc(wallet.rpc_port, "walletActiveAddress")["address"]
        txids = set()
        proving_started = time.monotonic()
        for index in range(args.transactions):
            sent = live.rpc(wallet.rpc_port, "walletSend", [address, 1_000, 0], timeout=300)
            live.require(sent["txid"] not in txids, "duplicate fixture txid")
            txids.add(sent["txid"])
            if (index + 1) % 32 == 0 or index + 1 == args.transactions:
                print(f"[wallet] {index + 1}/{args.transactions} real proofs", flush=True)
        record("wallet_proofs", {"seconds": time.monotonic() - proving_started,
                                 "txids": sorted(txids)})
        wait_pool(provider, txids, "hot_direct_and_gossip_delivery")
        live.require(provider.height() == height, "unexpected mining during pending test")

        cold = Node("cold", 2, args.new_bin)
        cold.start("fresh-node-cold-mempool", seeds=[provider.seed])
        converge(provider, cold, height)
        wait_pool(cold, txids, "cold_missing_only_recovery")
        cold.stop()
        cold.start("durable-state-empty-mempool-restart", seeds=[provider.seed])
        converge(provider, cold, height)
        wait_pool(cold, txids, "restarted_missing_only_recovery")
        if args.recovery_only:
            report["status"] = "passed"
            print("[PASS] real cold recovery and restart", flush=True)
            return

        mixed = Node("mixed", 3, args.new_bin)
        mixed.start("upgraded-client-legacy-bootstrap", seeds=[wallet.seed])
        converge(provider, mixed, height)
        # Other upgraded peers may subsequently be discovered. The assertion
        # is real negotiated v3 admission, not an artificial topology claim.
        live.wait_value("legacy response accepted by upgraded client",
                        lambda: "supports_missing=false" in mixed.log_text(), 180)
        record("legacy_negotiation", {"accepted": True,
                                      "pending": live.rpc(mixed.rpc_port, "getMempoolSize")})

        # A cold miner must refill through the new path before the test-only
        # gate opens. This does not change transaction or block admission.
        provider.stop()
        gate = base / "mining.ready"
        live.require(not gate.exists(), "mining gate already open")
        os.environ["NOID_ISOLATED_MINER_START_GATE"] = str(gate)
        os.environ["NOID_ISOLATED_FORCE_B255"] = "1"
        try:
            provider.start("cold-miner-recovery-before-pow", mode="miner", genesis=True,
                           seeds=[cold.seed, wallet.seed])
        finally:
            os.environ.pop("NOID_ISOLATED_MINER_START_GATE", None)
            os.environ.pop("NOID_ISOLATED_FORCE_B255", None)
        wait_pool(provider, txids, "cold_miner_ready")
        live.require(provider.height() == height, "closed mining gate failed")
        gate.touch()
        result = live.wait_mined(provider, height + 2, timeout=1800)
        provider.stop()
        provider.start("verify-confirmed-recovery", seeds=[cold.seed, wallet.seed])
        final_height = int(result["height"])
        for node in (cold, wallet, mixed):
            converge(provider, node, final_height)
            live.wait_value(f"{node.name} removes confirmed intents",
                            lambda node=node: live.rpc(node.rpc_port, "getMempoolSize") == 0,
                            180)
        included = collections.Counter()
        for txid in txids:
            confirmed = live.rpc(cold.rpc_port, "getTx", [txid])
            live.require(confirmed is not None, "recovered transaction was lost")
            included[int(confirmed["height"])] += 1
        live.require(dict(included) == {height + 1: 255, height + 2: 1},
                     f"wrong full-block/spillover partition {included}")
        record("confirmation_during_recovery", {"height": final_height, "included": dict(included)})
        report["status"] = "passed"
        print("[PASS] real cold recovery, restart, legacy negotiation and confirmation", flush=True)
    except BaseException as error:
        report["status"] = "failed"
        report["error"] = repr(error)
        raise
    finally:
        for node in reversed(nodes):
            if node.proc is not None:
                try:
                    node.sample()
                    node.stop()
                except Exception as error:
                    report.setdefault("cleanup_errors", []).append(str(error))
                    report["status"] = "failed"
        report["peak_rss_kib"] = {node.name: node.peak_rss_kib for node in nodes}
        save()


if __name__ == "__main__":
    main()

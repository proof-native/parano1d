#!/usr/bin/env python3
"""Finish the H5 full-block and aligned-snapshot tests with a warm miner.

The held relay owns pending test intents across the interrupted cold-start
run. Its namespace must have loopback only. The isolated producer waits for a
test-only start marker while normal wallet/RPC/P2P traffic fills its mempool.
No database, proof, admission rule or synchronization limit is bypassed.
"""

import argparse
import json
import os
import re
import time
from pathlib import Path

import live_v1_1_activation_scenario as audit


rpc = audit.rpc
require = audit.require
record = audit.record


def scenario(args):
    held_port = args.held_relay_rpc
    held_seed = f"127.0.0.1:{held_port - 1}"
    held_chain = rpc(held_port, "getChainInfo")
    held = rpc(held_port, "getMempoolInfo")
    tip = int(held_chain["height"])
    require(tip == 56 and held["size"] == 40, "held recovery checkpoint changed")
    txids = [tx["tx_hash"] for tx in held["txs"]]
    require(len(txids) == 40 and all(tx["page_count"] == 1 for tx in held["txs"]),
            "unexpected held intent shapes")
    record("held_checkpoint", {"chain": held_chain, "pending_txids": txids})
    gate = audit.BASE / "warm-capacity.ready"
    require(not gate.exists(), "refusing to reuse an open start gate")

    a = audit.Node("a", 0, producer=True)
    b = audit.Node("b", 1)
    wallet = audit.Node("capacity-wallet", 5)
    old_gate = os.environ.get("NOID_ISOLATED_MINER_START_GATE")
    os.environ["NOID_ISOLATED_MINER_START_GATE"] = str(gate)
    try:
        a.start("a-waits-for-complete-mempool", mode="miner", genesis=True, seeds=[held_seed])
    finally:
        if old_gate is None:
            os.environ.pop("NOID_ISOLATED_MINER_START_GATE", None)
        else:
            os.environ["NOID_ISOLATED_MINER_START_GATE"] = old_gate
    b.start("b-full-capacity-verifier", seeds=[a.seed, held_seed])
    wallet.start("wallet-preserves-held-intents", seeds=[a.seed, b.seed, held_seed])
    audit.converge(a, b, tip)
    audit.converge(a, wallet, tip)
    for node in (a, b, wallet):
        audit.live.wait_value(f"{node.name} restores all held intents",
                              lambda node=node: rpc(node.rpc_port, "getMempoolSize") == len(txids), 300)
    address = rpc(wallet.rpc_port, "walletActiveAddress")["address"]
    for index in range(256 - len(txids)):
        sent = rpc(wallet.rpc_port, "walletSend", [address, 10_000, 0], timeout=300)
        require(sent["input_count"] == 1 and sent["output_count"] == 2, "new fixture shape changed")
        txids.append(sent["txid"])
        if (index + 1) % 32 == 0:
            print(f"[warm] {len(txids)}/256 intents", flush=True)
    for node in (a, b, wallet):
        audit.live.wait_value(f"{node.name} has the complete 256-intent workload",
                              lambda node=node: rpc(node.rpc_port, "getMempoolSize") == 256, 300)
    require(a.height() == tip, "closed start gate allowed mining")
    record("queued_before_start", {"tip": tip, "txids": txids, "count": len(txids)})
    opened = time.monotonic()
    gate.touch()
    result = audit.live.wait_mined(a, tip + 3, timeout=1800)
    elapsed = time.monotonic() - opened
    a.stop()
    require(int(result["height"]) == tip + 3, "miner overshot the empty-child check")
    a.start("a-restart-after-full-block", seeds=[b.seed, wallet.seed, held_seed])
    audit.converge(a, b, tip + 3)
    audit.converge(a, wallet, tip + 3)

    counts = {}
    for txid in txids:
        tx = rpc(b.rpc_port, "getTx", [txid])
        require(tx is not None, "a queued intent is not confirmed")
        height = int(tx["height"])
        counts[height] = counts.get(height, 0) + 1
    require(counts == {tip + 1: 255, tip + 2: 1}, f"wrong exact partition: {counts}")
    full = audit.detail(b, tip + 1)
    spill = audit.detail(b, tip + 2)
    empty = audit.detail(b, tip + 3)
    require(full["user_pages"] == 255 and full["proof_class"].startswith("B255"), "not a full B255")
    require(full["bundle_bytes"] > 1_048_576, "full bundle did not exceed 1 MiB")
    require(spill["user_pages"] == 1 and empty["user_pages"] == 0, "wrong smaller children")
    for node in (a, b, wallet):
        require(rpc(node.rpc_port, "getMempoolSize") == 0, "confirmed intents remain pending")
    mining_log = next(Path(entry["path"]) for entry in audit.LOGS
                      if "a-waits-for-complete-mempool" in entry["path"])
    text = mining_log.read_text()
    require("coinbase-only template: new tx admitted, cancelling PoW" not in text,
            "old admission notifications cancelled fresh PoW")
    record("full_block_and_spillover", {"full": full, "spill": spill, "empty": empty,
                                         "seconds_for_three_blocks": elapsed,
                                         "old_notifications_did_not_cancel_pow": True})

    # H54 is the already-mined B255 block on the published six-block grid.
    wallet.stop()
    a.stop()
    audit.mine(a, 72, seeds=[b.seed, held_seed])
    a.start("a-publishes-aligned-b255-snapshot", seeds=[b.seed, held_seed])
    audit.converge(a, b, 72)
    terminal = audit.terminal_at_finality(a, 54, 5)
    require(bytes.fromhex(rpc(a.rpc_port, "getHistoryStepTerminal"))[41] == 1, "H54 is not B255")
    newcomer = audit.Node("aligned-b255-snapshot", 6)
    newcomer.start("fresh-b255-boundary-snapshot", seeds=[a.seed])
    audit.converge(a, newcomer, 72, timeout=600)
    installed = re.search(r"snapshot installed[^\n]*boundary_height=(\d+)", newcomer.log_text())
    require(installed is not None and int(installed.group(1)) == 54, "wrong installed boundary")
    evidence = json.loads((audit.BASE / "boundary.json").read_text())["old_payment"]
    audit.verify_old_payment(newcomer, evidence)
    require(rpc(newcomer.rpc_port, "getBlock", [2]) is None, "fresh node unexpectedly retains H2 body")
    record("b255_snapshot", {"tip": 72, "boundary": 54, "terminal_bytes": terminal["bytes"],
                             "old_receipt_verified": True})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--test-bin", type=Path, required=True)
    parser.add_argument("--producer-bin", type=Path, required=True)
    parser.add_argument("--held-relay-rpc", type=int, default=28171)
    args = parser.parse_args()
    audit.BASE = args.run_dir.resolve()
    audit.TEST_BIN = args.test_bin.resolve()
    audit.PRODUCER_BIN = args.producer_bin.resolve()
    audit.PHASE = "warm-capacity"
    audit.live.BASE = audit.BASE
    audit.receipt.BASE = audit.BASE
    require(not (audit.BASE / "warm-capacity.json").exists(), "refusing to overwrite evidence")
    interfaces = [line.split(':')[0].strip() for line in Path("/proc/net/dev").read_text().splitlines()[2:]]
    require(interfaces == ["lo"], "loopback-only namespace required")
    original = json.loads((audit.BASE / "boundary.json").read_text())
    require(original["test_binary_sha256"] == audit.live.sha256(audit.TEST_BIN), "checking binary changed")
    audit.SUMMARY = {"status": "running", "activation_height": 5, "network_interfaces": interfaces,
                     "test_binary_sha256": audit.live.sha256(audit.TEST_BIN),
                     "producer_binary_sha256": audit.live.sha256(audit.PRODUCER_BIN)}
    audit.save()
    try:
        scenario(args)
        for node in reversed(audit.NODES):
            node.stop()
        audit.audit_logs()
        audit.SUMMARY["status"] = "passed"
        print("[PASS] warm full-capacity, notification regression and B255 snapshot", flush=True)
    except BaseException as error:
        audit.SUMMARY["status"] = "failed"
        audit.SUMMARY["error"] = repr(error)
        raise
    finally:
        for node in reversed(audit.NODES):
            if node.proc is not None and node.proc.poll() is None:
                try:
                    node.stop()
                except Exception as error:
                    audit.SUMMARY.setdefault("cleanup_errors", []).append(str(error))
                    audit.SUMMARY["status"] = "failed"
        audit.save()


if __name__ == "__main__":
    main()

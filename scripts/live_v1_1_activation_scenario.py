#!/usr/bin/env python3
"""Isolated H=5 fork, reorg, sync, shared-path wire and receipt acceptance test.

Build with `--features noid_chain/isolated-v1-1-testnet` into a separate target
directory. Run each phase inside `unshare -Urn` after bringing loopback up.
No public seeds, existing wallet directories, matrix changes or mainnet data
are used. Run boundary, extended, workloads, then capacity. Boundary creates the chain;
extended checks finalized terminals and sync gaps. Workloads requires the
separately built isolated_v1_1_node example as --producer-bin, so even a slow
machine can exercise B255 after activation. Verifier nodes have no override.
Before height 5 only B25 fits the legacy terminal limit.
Capacity funds a separate test wallet and exercises the exact 255-page block
limit with 256 independent queued payments, including the one-page spillover.
The largest batch uses the isolated producer's start gate so it does not
assume that bounded cold-start mempool exchange recovers 256 intents at once.
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import time
from pathlib import Path

import live_two_miner_fork_reorg_scenario as live
import live_receipt_lifecycle_scenario as receipt


require = live.require
rpc = live.rpc
NODES = []
LOGS = []
BASE = None
SUMMARY = None
PHASE = None
SEQUENCE = 0
TEST_BIN = None
LEGACY_BIN = None
PRODUCER_BIN = None


class Node(live.Node):
    def __init__(self, name, index, legacy=False, producer=False):
        super().__init__(name, 28100 + 10 * index, 28101 + 10 * index)
        self.binary = PRODUCER_BIN if producer else LEGACY_BIN if legacy else TEST_BIN
        self.legacy = legacy
        self.producer = producer
        self.peak_rss_kib = 0
        NODES.append(self)

    def sample_memory(self):
        if self.proc is None:
            return
        try:
            for line in Path(f"/proc/{self.proc.pid}/status").read_text().splitlines():
                if line.startswith("VmHWM:"):
                    self.peak_rss_kib = max(self.peak_rss_kib, int(line.split()[1]))
        except OSError:
            pass

    def info(self, timeout=15):
        self.sample_memory()
        return super().info(timeout)

    def request_stop(self):
        self.sample_memory()
        return super().request_stop()

    def start(self, label, **kwargs):
        global SEQUENCE
        SEQUENCE += 1
        label = f"{PHASE}-{SEQUENCE:02d}-{label}"
        require(not (BASE / "logs" / f"{label}.log").exists(), "refusing to overwrite a log")
        original = live.NODE_BIN
        original_force = os.environ.get("NOID_ISOLATED_FORCE_B255")
        live.NODE_BIN = self.binary
        if self.producer and kwargs.get("mode") == "miner":
            os.environ["NOID_ISOLATED_FORCE_B255"] = "1"
        else:
            os.environ.pop("NOID_ISOLATED_FORCE_B255", None)
        try:
            result = super().start(label, **kwargs)
            LOGS.append({"path": str(self.log_path), "legacy": self.legacy})
            return result
        finally:
            live.NODE_BIN = original
            if original_force is None:
                os.environ.pop("NOID_ISOLATED_FORCE_B255", None)
            else:
                os.environ["NOID_ISOLATED_FORCE_B255"] = original_force


def save():
    (BASE / f"{PHASE}.json").write_text(json.dumps(SUMMARY, indent=2) + "\n")


def record(name, value):
    SUMMARY[name] = value
    save()
    print(f"[checkpoint] {name}", flush=True)


def mine(node, target, seeds=()):
    require(node.proc is None or node.proc.poll() is not None, "miner is still running")
    node.start(f"{node.name}-mine-to-{target}", mode="miner", genesis=True, seeds=list(seeds))
    result = live.wait_mined(node, target, timeout=2400)
    node.stop()
    require(int(result["height"]) == target, f"miner overshot h{target}: {result}")
    record(f"mined_{node.name}_{target}", result)
    return result


def converge(source, destination, target, timeout=300):
    result = live.wait_value(
        f"{destination.name} exact h{target}",
        lambda: live.exact_tip(source, destination),
        timeout=timeout,
    )
    require(int(result[source.name]) == target, f"wrong convergence height: {result}")
    require(live.exact_headers(source, destination, target), "permanent headers differ")
    source_state = rpc(source.rpc_port, "getActiveSlotCount")
    require(source_state == rpc(destination.rpc_port, "getActiveSlotCount"), "live counts differ")
    record(f"converged_{destination.name}_{target}", result)


def verify_old_payment(node, evidence):
    result = rpc(node.rpc_port, "verifyReceipt", [evidence["receipt_hex"]])
    receipt.assert_verified(
        result, evidence["txid"], evidence["height"], evidence["sender"],
        evidence["recipient"], evidence["amount"], evidence["fee"], node.name,
    )
    return result


def detail(node, height):
    data = rpc(node.rpc_port, "getBlockDetails", [height])
    require(data and data["retained"], f"missing retained block h{height}")
    retained = data["retained"]
    cap = 1_048_576 if height < 5 else 1_100_000
    require(int(retained["history_step_bytes"]) <= cap, "terminal exceeds height-selected cap")
    # Keep measurements, not the complete public transaction listing.
    return {k: v for k, v in retained.items() if k != "transactions"}


def audit_logs():
    failures = []
    warnings = []
    queue_peaks = {}
    for entry in LOGS:
        text = Path(entry["path"]).read_text(errors="replace")
        require("embedded DNS bootstrap disabled" in text, "DNS bootstrap was not disabled")
        require("dialing embedded seed" not in text, "embedded seed was dialed")
        for line in text.splitlines():
            if "P2P reactor health" in line:
                for key, value in re.findall(r"(\w+)=(\d+)", line):
                    queue_peaks[key] = max(queue_peaks.get(key, 0), int(value))
            if " WARN " in line:
                warnings.append({"log": entry["path"], "line": line})
            markers = ("panicked", " ERROR ") if entry["legacy"] else (
                "panicked", " ERROR ", "reorg failed", "P2P block rejected",
                "chained orphan apply failed", "snapshot install failed",
                "snapshot generation build failed", "wallet receipt recovery",
            )
            if any(marker in line for marker in markers):
                failures.append({"log": entry["path"], "line": line})
    record("log_review", {"logs": LOGS, "warnings": warnings, "failures": failures,
                          "sampled_queue_peaks": queue_peaks,
                          "node_peak_rss_kib": {node.name: node.peak_rss_kib for node in NODES}})
    require(not failures, f"unexpected failures in logs: {failures[-5:]}")


def boundary():
    a = Node("a", 0)
    b = Node("b", 1)
    c = Node("c", 2)
    old = Node("legacy", 3, legacy=True)

    mine(a, 1)
    a.start("a-funded-node", genesis=True)
    b.start("b-fresh-payment-recipient", seeds=[a.seed])
    converge(a, b, 1)
    sender = rpc(a.rpc_port, "walletActiveAddress")["address"]
    recipient = rpc(b.rpc_port, "walletActiveAddress")["address"]
    sent = rpc(a.rpc_port, "walletSend", [recipient, 10_000, 0], timeout=300)
    live.wait_value("payment reaches relay", lambda: rpc(b.rpc_port, "getMempoolSize") == 1, 180)
    a.stop()
    mine(a, 3, seeds=[b.seed])
    a.start("a-common-prefix", genesis=True, seeds=[b.seed])
    converge(a, b, 3)
    tx = receipt.wait_tx(a, sent["txid"], timeout=15)
    require(int(tx["height"]) < 5, "payment was not pre-fork")
    evidence = {
        "receipt_hex": receipt.export_receipt(a, sent["txid"]),
        "txid": sent["txid"], "height": int(tx["height"]), "sender": sender,
        "recipient": recipient, "amount": 10_000, "fee": int(sent["fee_micronoid"]),
    }
    verify_old_payment(a, evidence)
    verify_old_payment(b, evidence)
    record("old_payment", evidence)
    b.stop()
    a.stop()

    # All three start from bytes transported by P2P, not copied databases.
    mine(a, 4)
    a.start("a-h4-server")
    c.start("c-pre-fork", seeds=[a.seed])
    converge(a, c, 4)
    old.start("legacy-accepts-h4", seeds=[a.seed])
    converge(a, old, 4)
    record("pre_fork_h4", detail(a, 4))
    old.stop()
    c.stop()
    a.stop()

    # B's competing branch includes both a legacy and a shared-path terminal.
    mine(b, 5)
    mine(a, 5)
    a.start("a-restart-at-fork")
    c.start("c-crosses-fork", seeds=[a.seed])
    converge(a, c, 5)
    record("activation_h5", detail(a, 5))
    verify_old_payment(c, evidence)
    c.stop()  # h5 -> h24 later is exactly a 19-block gap.
    a.stop()

    mine(a, 6)
    a.start("a-h6-winning-branch")
    b.start("b-reorg-through-activation", seeds=[a.seed])
    converge(a, b, 6)
    require("reorg complete" in b.log_text(), "missing reorg evidence")
    require("requesting snapshot" not in b.log_text(), "shallow reorg used snapshot")
    verify_old_payment(b, evidence)
    record("post_fork_h6", detail(b, 6))
    b.stop()  # h6 -> h24 later is exactly an 18-block gap.

    old.start("legacy-refuses-post-fork", seeds=[a.seed])
    live.wait_value("legacy peer connects to post-fork source", lambda: rpc(old.rpc_port, "getPeerCount") >= 1, 120)
    deadline = time.monotonic() + 35
    while time.monotonic() < deadline:
        require(old.height() == 4, "legacy node installed a post-fork State")
        time.sleep(0.5)
    rejection_lines = [line for line in old.log_text().splitlines()
                       if any(word in line.lower() for word in ("reject", "invalid", "precheck failed"))]
    require(rejection_lines, "legacy node has no evidence of rejecting post-fork data")
    record("legacy_rejection", {"height": old.height(), "log": str(old.log_path), "rejections": rejection_lines})
    old.stop()
    a.stop()


def terminal_at_finality(node, expected_height, version):
    encoded = rpc(node.rpc_port, "getHistoryStepTerminal")
    require(encoded, "missing finalized terminal")
    raw = bytes.fromhex(encoded)
    require(raw[0] == version, f"wrong terminal wire version at h{expected_height}")
    require(int.from_bytes(raw[1:9], "little") == expected_height, "wrong terminal height")
    result = {"height": expected_height, "version": raw[0], "bytes": len(raw),
              "sha256": hashlib.sha256(raw).hexdigest()}
    record(f"finalized_terminal_{expected_height}", result)
    return result


def payment_batch(a, b, count):
    a.start(f"a-queue-{count}", genesis=True, seeds=[b.seed])
    converge(a, b, a.height())
    active = rpc(a.rpc_port, "walletActiveAddress")["address"]
    require(int(rpc(a.rpc_port, "walletGetBalance")["utxo_count"]) >= count, "not enough test inputs")
    sends = []
    # Each split round lowers the payment amount so outputs from the previous
    # round can independently fund the next payment plus its consensus fee.
    amount = {26: 1_000_000, 32: 100_000, 64: 10_000}[count]
    for index in range(count):
        sent = rpc(a.rpc_port, "walletSend", [active, amount, 0], timeout=300)
        require(sent["input_count"] == 1 and sent["output_count"] == 2, "split fixture lost its 1-to-2 shape")
        sends.append(sent)
        if index % 16 == 15:
            print(f"[batch] {index + 1}/{count} proved", flush=True)
    live.wait_value("whole batch reaches relay", lambda: rpc(b.rpc_port, "getMempoolSize") == count, 300)
    a.stop()
    a.start(f"a-mines-{count}", mode="miner", genesis=True, seeds=[b.seed])
    txids = [sent["txid"] for sent in sends]
    confirmed = []

    def all_confirmed():
        nonlocal confirmed
        confirmed = [rpc(a.rpc_port, "getTx", [txid]) for txid in txids]
        return all(confirmed)

    live.wait_value(f"{count} payments confirmed", all_confirmed, 1800, interval=1)
    final_height = a.height()
    a.stop()
    a.start(f"a-after-batch-{count}", seeds=[b.seed])
    converge(a, b, final_height)
    heights = sorted({int(tx["height"]) for tx in confirmed})
    blocks = {str(height): detail(b, height) for height in heights}
    require(any("B255" in data["proof_class"] for data in blocks.values()), "batch did not exercise B255")
    record(f"batch_{count}", {"blocks": blocks, "txids": txids, "tip": final_height, "amount": amount})
    a.stop()
    return final_height


def extended():
    prior = json.loads((BASE / "boundary.json").read_text())
    require(prior["status"] == "passed", "boundary phase did not pass")
    require(prior["test_binary_sha256"] == live.sha256(TEST_BIN), "test binary changed between phases")
    evidence = prior["old_payment"]
    a = Node("a", 0)
    b = Node("b", 1)
    c = Node("c", 2)
    fresh = Node("fresh-snapshot", 4)

    mine(a, 22)
    a.start("a-finalized-legacy-h4")
    terminal_at_finality(a, 4, 4)
    verify_old_payment(a, evidence)
    require(receipt.export_receipt(a, evidence["txid"]) == evidence["receipt_hex"], "old receipt changed")
    a.stop()
    mine(a, 23)
    a.start("a-finalized-shared-h5")
    terminal_at_finality(a, 5, 5)
    a.stop()
    mine(a, 24)
    a.start("a-h24-snapshot-server")
    terminal_at_finality(a, 6, 5)

    b.start("b-gap18-retained-sync", seeds=[a.seed])
    converge(a, b, 24)
    require("snapshot installed" not in b.log_text(), "18-block extension unexpectedly installed snapshot")
    c.start("c-gap19-snapshot-sync", seeds=[a.seed])
    converge(a, c, 24, timeout=600)
    require("snapshot installed" in c.log_text(), "19-block boundary did not install a snapshot")
    fresh.start("fresh-post-fork-snapshot-sync", seeds=[a.seed])
    converge(a, fresh, 24, timeout=600)
    require("snapshot installed" in fresh.log_text(), "fresh join did not install a snapshot")
    for node in (b, c, fresh):
        verify_old_payment(node, evidence)
    record("gap_boundary_logs", {"18": str(b.log_path), "19": str(c.log_path), "fresh": str(fresh.log_path)})
    c.stop()
    fresh.stop()
    a.stop()

def finish_workloads(a, b, fresh, evidence):
    mine(a, 30, seeds=[b.seed])
    # Grow only our own temporary wallet's independent inputs, then exercise
    # larger exact-object responses and B255 with real wallet proofs. Current
    # production gossip is header-only; no inline-bundle path is in use.
    for count in (26, 32, 64):
        payment_batch(a, b, count)
    a.start("a-batch-summary", seeds=[b.seed])
    current = a.height()
    a.stop()
    # The first following B25 proof must recurse over the completed B255.
    mine(a, current + 1, seeds=[b.seed])
    a.start("a-b25-after-b255", seeds=[b.seed])
    converge(a, b, current + 1)
    record("b25_after_b255", detail(b, current + 1))
    a.stop()

    prune_height = max(current + 2, evidence["height"] + 43)
    mine(a, prune_height, seeds=[b.seed])
    a.start("a-post-prune-restart", seeds=[b.seed])
    converge(a, b, prune_height)
    for node in (a, b):
        require(rpc(node.rpc_port, "getBlock", [evidence["height"]]) is None, "old body not pruned")
        verify_old_payment(node, evidence)
    require(receipt.export_receipt(a, evidence["txid"]) == evidence["receipt_hex"], "pruning changed receipt")
    receipt.content_tamper(b, evidence["receipt_hex"])
    corrupted = bytearray.fromhex(evidence["receipt_hex"])
    corrupted[-1] ^= 1
    verdict = rpc(b.rpc_port, "verifyReceipt", [corrupted.hex()])
    require(not verdict["canonical"] and not verdict["confirmed"], "wrong-chain receipt accepted")
    record("post_prune_receipt", {"source_height": evidence["height"], "tip": prune_height, "unchanged": True})

    fresh.start("snapshot-node-offline-catchup-and-restart", seeds=[a.seed])
    converge(a, fresh, prune_height, timeout=600)
    verify_old_payment(fresh, evidence)
    checked = subprocess.run(
        [str(TEST_BIN.with_name("parano1d-cli")), "--rpc", f"http://127.0.0.1:{fresh.rpc_port}",
         "--json", "verify", evidence["receipt_hex"]],
        capture_output=True, text=True, timeout=120, check=True,
    )
    require(json.loads(checked.stdout)["confirmed"], "CLI receipt verification failed")
    record("independent_cli_receipt", {"confirmed": True})


def workloads():
    prior = json.loads((BASE / "boundary.json").read_text())
    sync = json.loads((BASE / "extended.json").read_text())
    require(prior["status"] == "passed", "boundary phase did not pass")
    require(prior["test_binary_sha256"] == live.sha256(TEST_BIN), "verifier binary changed")
    require(PRODUCER_BIN is not None and PRODUCER_BIN.is_file(), "missing isolated workload producer")
    for name in ("converged_b_24", "converged_c_24", "converged_fresh-snapshot_24", "gap_boundary_logs"):
        require(name in sync, f"sync prerequisite missing: {name}")
    for height, version in ((4, 4), (5, 5), (6, 5)):
        require(sync[f"finalized_terminal_{height}"]["version"] == version, "missing finalized wire transition")
    for path in sorted((BASE / "logs").glob("extended-*.log")):
        LOGS.append({"path": str(path), "legacy": False})
    record("sync_prerequisites", {"source": str(BASE / "extended.json"), "checked": True})
    record("producer_binary_sha256", live.sha256(PRODUCER_BIN))
    a = Node("a", 0, producer=True)
    b = Node("b", 1)
    fresh = Node("fresh-snapshot", 4)
    b.start("b-retains-pending-payments")
    finish_workloads(a, b, fresh, prior["old_payment"])


def capacity():
    prior = json.loads((BASE / "workloads.json").read_text())
    require(prior["status"] == "passed", "workloads did not pass")
    require(prior["test_binary_sha256"] == live.sha256(TEST_BIN), "verifier binary changed")
    require(PRODUCER_BIN is not None and PRODUCER_BIN.is_file(), "missing isolated producer")
    require(prior["producer_binary_sha256"] == live.sha256(PRODUCER_BIN), "producer binary changed")
    record("producer_binary_sha256", live.sha256(PRODUCER_BIN))
    resume = PHASE == "capacity-resume"
    if resume:
        interrupted = json.loads((BASE / "capacity.json").read_text())
        require(interrupted["status"] == "failed"
                and interrupted["error"] == "snapshot used a different proof class",
                "resume is only for the preserved snapshot-alignment fixture failure")
        require(interrupted["test_binary_sha256"] == live.sha256(TEST_BIN), "checking binary changed")
        require(interrupted["producer_binary_sha256"] == live.sha256(PRODUCER_BIN), "producer changed")
        for key in ("capacity_funding", "capacity_1", "capacity_2", "capacity_4"):
            record(key, interrupted[key])
        for path in sorted((BASE / "logs").glob("capacity-*.log")):
            LOGS.append({"path": str(path), "legacy": False})
        record("resumed_checkpoints", {"source": str(BASE / "capacity.json"), "next_split_count": 8})
    a = Node("a", 0, producer=True)
    b = Node("b", 1)
    wallet = Node("capacity-wallet", 5)
    a.start("a-funds-capacity-wallet")
    tip = a.height()
    require(tip >= 5, "capacity workload is post-fork only")
    b.start("b-capacity-verifier", seeds=[a.seed])
    wallet.start("independent-capacity-wallet", seeds=[a.seed, b.seed])
    converge(a, b, tip)
    converge(a, wallet, tip, timeout=600)
    address = rpc(wallet.rpc_port, "walletActiveAddress")["address"]
    if resume:
        require(tip == 49 and rpc(wallet.rpc_port, "walletGetBalance")["utxo_count"] == 8,
                "preserved capacity checkpoint changed")
    else:
        require(rpc(wallet.rpc_port, "walletGetBalance")["utxo_count"] == 0, "capacity wallet was not fresh")
        funding = rpc(a.rpc_port, "walletSend", [address, 25_000_000, 0], timeout=300)
        live.wait_value("funding reaches relay", lambda: rpc(b.rpc_port, "getMempoolSize") == 1, 180)

    def confirm_batch(txids, label, start_gate=None):
        if start_gate is None:
            a.stop()
            a.start(f"a-mines-{label}", mode="miner", genesis=True, seeds=[b.seed, wallet.seed])
        else:
            require(not start_gate.exists(), "capacity gate opened before all intents arrived")
            start_gate.touch()
        transactions = []

        def included():
            nonlocal transactions
            transactions = [rpc(a.rpc_port, "getTx", [txid]) for txid in txids]
            return all(transactions)

        live.wait_value(f"{label} confirmed", included, 1800, interval=1)
        height = a.height()
        a.stop()
        a.start(f"a-verifies-{label}", seeds=[b.seed, wallet.seed])
        converge(a, b, height)
        converge(a, wallet, height)
        blocks = {str(h): detail(b, h) for h in sorted({int(tx["height"]) for tx in transactions})}
        require(all(block["user_pages"] <= 255 for block in blocks.values()), "page cap exceeded")
        return blocks

    if not resume:
        record("capacity_funding", confirm_batch([funding["txid"]], "funding"))
    # Equal-value binary splitting keeps every next-round input large enough
    # to pay its own fee, without unconfirmed chains or multi-input payments.
    for count in (1, 2, 4, 8, 16, 32, 64, 128, 256):
        if resume and count < 8:
            continue
        utxos = rpc(wallet.rpc_port, "walletListUtxos")
        require(len(utxos) == count and not any(u["reserved"] for u in utxos), "unexpected split inputs")
        minimum = min(int(u["value_micronoid"]) for u in utxos)
        amount = (minimum - 9_000) // 2
        require(amount > 0, "split fixture exhausted its funding")
        start_gate = None
        queued_at = a.height()
        if count == 256:
            a.stop()
            start_gate = BASE / f"{PHASE}-full-capacity.ready"
            require(not start_gate.exists(), "refusing to reuse a full-capacity start gate")
            previous_gate = os.environ.get("NOID_ISOLATED_MINER_START_GATE")
            os.environ["NOID_ISOLATED_MINER_START_GATE"] = str(start_gate)
            try:
                a.start("a-waits-for-full-capacity", mode="miner", genesis=True,
                        seeds=[b.seed, wallet.seed])
            finally:
                if previous_gate is None:
                    os.environ.pop("NOID_ISOLATED_MINER_START_GATE", None)
                else:
                    os.environ["NOID_ISOLATED_MINER_START_GATE"] = previous_gate
        txids = []
        for index in range(count):
            sent = rpc(wallet.rpc_port, "walletSend", [address, amount, 0], timeout=300)
            require(sent["input_count"] == 1 and sent["output_count"] == 2, "split shape changed")
            require(int(sent["fee_micronoid"]) == 9_000, "split fee changed")
            txids.append(sent["txid"])
            if index % 32 == 31:
                print(f"[capacity] {index + 1}/{count} wallet proofs", flush=True)
        for node in (a, b):
            live.wait_value(f"{node.name} receives {count} payments",
                            lambda node=node: rpc(node.rpc_port, "getMempoolSize") == count, 300)
        require(a.height() == queued_at, "miner advanced before the batch was complete")
        record(f"queued_capacity_{count}", {"height": queued_at, "txids": txids,
                                            "amount": amount, "warm_start": start_gate is not None})
        blocks = confirm_batch(txids, f"capacity-{count}", start_gate)
        require(sum(block["user_pages"] for block in blocks.values()) == count, "wrong included page count")
        record(f"capacity_{count}", {"blocks": blocks, "amount": amount, "txids": txids})
        if count == 256:
            require(sorted(block["user_pages"] for block in blocks.values()) == [1, 255],
                    "256 payments did not exercise an exact full block and spillover")
            full = next(block for block in blocks.values() if block["user_pages"] == 255)
            require(full["proof_class"].startswith("B255"), "full block has wrong class")
            require(full["bundle_bytes"] > 1_048_576, "fixture did not produce a full bundle above 1 MiB")
            require(full["history_step_bytes"] <= 1_100_000, "terminal cap exceeded")
            for node in (a, b, wallet):
                require(rpc(node.rpc_port, "getMempoolSize") == 0, "confirmed workload left pending entries")
            record("full_block_and_spillover", {"passed": True, "full_block": full})

    # Production snapshot boundaries are every six blocks, independently of
    # the latest finalized terminal exposed by RPC. Select a real B255 block
    # on that grid, then let it cross finality without changing sync policy.
    aligned = [int(h) for value in SUMMARY.values() if isinstance(value, dict)
               for h, block in value.get("blocks", {}).items()
               if int(h) % 6 == 0 and block["proof_class"].startswith("B255")]
    require(aligned, "fixture did not produce B255 on a snapshot boundary")
    snapshot_height = max(aligned)
    snapshot_tip = snapshot_height + 18
    require(a.height() <= snapshot_tip, "fixture passed the selected snapshot window")
    wallet.stop()
    a.stop()
    mine(a, snapshot_tip, seeds=[b.seed])
    a.start("a-aligned-b255-snapshot-server", seeds=[b.seed])
    converge(a, b, snapshot_tip)
    finalized = terminal_at_finality(a, snapshot_height, 5)
    require(bytes.fromhex(rpc(a.rpc_port, "getHistoryStepTerminal"))[41] == 1,
            "aligned finalized terminal is not B255")
    newcomer = Node("aligned-b255-snapshot", 6)
    newcomer.start("fresh-aligned-b255-terminal-snapshot", seeds=[a.seed])
    converge(a, newcomer, snapshot_tip, timeout=600)
    installed = re.search(r"snapshot installed[^\n]*boundary_height=(\d+)", newcomer.log_text())
    require(installed is not None and int(installed.group(1)) == snapshot_height,
            "snapshot did not use the selected B255 boundary")
    evidence = json.loads((BASE / "boundary.json").read_text())["old_payment"]
    verify_old_payment(newcomer, evidence)
    record("b255_snapshot", {"tip": snapshot_tip, "snapshot_boundary_height": snapshot_height,
                             "terminal_bytes": finalized["bytes"], "old_receipt_verified": True})


def main():
    global BASE, PHASE, SUMMARY, TEST_BIN, LEGACY_BIN, PRODUCER_BIN
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--test-bin", type=Path, required=True)
    parser.add_argument("--legacy-bin", type=Path, required=True)
    parser.add_argument("--producer-bin", type=Path)
    parser.add_argument("--phase", choices=("boundary", "extended", "workloads", "capacity", "capacity-resume"), required=True)
    args = parser.parse_args()
    BASE = args.run_dir.resolve()
    TEST_BIN = args.test_bin.resolve()
    LEGACY_BIN = args.legacy_bin.resolve()
    PRODUCER_BIN = args.producer_bin.resolve() if args.producer_bin else None
    PHASE = args.phase
    require(TEST_BIN.is_file() and LEGACY_BIN.is_file(), "missing executable")
    interfaces = [line.split(':')[0].strip() for line in Path("/proc/net/dev").read_text().splitlines()[2:]]
    require(interfaces == ["lo"], "test requires a loopback-only network namespace")
    if PHASE == "boundary":
        require(not BASE.exists(), "boundary test requires a fresh run directory")
        (BASE / "logs").mkdir(parents=True)
    else:
        require((BASE / "boundary.json").is_file(), "missing successful boundary run")
        require(not (BASE / f"{PHASE}.json").exists(), "refusing to overwrite phase run")
    live.BASE = BASE
    receipt.BASE = BASE
    SUMMARY = {
        "status": "running", "phase": PHASE, "activation_height": 5,
        "network_interfaces": interfaces, "test_binary_sha256": live.sha256(TEST_BIN),
        "legacy_binary_sha256": live.sha256(LEGACY_BIN), "run_dir": str(BASE),
    }
    save()
    try:
        {"boundary": boundary, "extended": extended, "workloads": workloads,
         "capacity": capacity, "capacity-resume": capacity}[PHASE]()
        for node in reversed(NODES):
            node.stop()
        audit_logs()
        SUMMARY["status"] = "passed"
        print(f"[PASS] v1.1 H5 {PHASE}", flush=True)
    except BaseException as error:
        SUMMARY["status"] = "failed"
        SUMMARY["error"] = str(error)
        raise
    finally:
        cleanup = []
        for node in reversed(NODES):
            if node.proc is not None and node.proc.poll() is None:
                try:
                    node.stop()
                except Exception as error:
                    cleanup.append(f"{node.name}: {error}")
        if cleanup:
            SUMMARY["cleanup_errors"] = cleanup
            SUMMARY["status"] = "failed"
        save()
        print(f"[summary] {BASE / (PHASE + '.json')}", flush=True)


if __name__ == "__main__":
    main()

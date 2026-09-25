#!/usr/bin/env python3
"""Find watched contract states after another participant calls and after reorg.

Run in a loopback-only namespace. NOID_V2_DISCOVERY_SOURCE must identify the
passed, stopped shared-receipt fixture; NOID_V2_LIVE_DIR must be fresh.
"""
import json
import os
from pathlib import Path
import secrets
import subprocess
import time

import live_v2_contract_scenario as contracts

live = contracts.live
BASE = contracts.BASE
ROOT = contracts.ROOT
rpc = contracts.rpc


def main():
    devices = [line.split(":")[0].strip() for line in Path("/proc/net/dev").read_text().splitlines()[2:]]
    live.require(devices == ["lo"], "use an isolated loopback-only network namespace")
    subprocess.run(["ip", "link", "set", "lo", "up"], check=True)
    source = Path(os.environ["NOID_V2_DISCOVERY_SOURCE"]).resolve()
    prior = json.loads((source / "report.json").read_text())
    live.require(prior["status"] == "passed" and not prior.get("shutdown_errors"), "source did not pass and stop")
    live.require(not BASE.exists(), f"fresh directory required: {BASE}")
    BASE.mkdir(parents=True)
    (BASE / "logs").mkdir()
    for name in ("producer", "receiver"):
        subprocess.run(["cp", "-a", "--reflink=auto", "--sparse=always", str(source / name), str(BASE / name)], check=True)
    key = BASE / "mining.key"
    with key.open("x") as file:
        file.write(secrets.token_hex(32) + "\n")
    key.chmod(0o600)
    live.BASE = BASE
    a = contracts.Node("producer", 26300, 26301)
    b = contracts.Node("receiver", 26310, 26311)
    alternate = contracts.Node("alternate", 26320, 26321)
    report = {"status": "running", "source_tip": prior["final_tip"], "observations": [],
              "source_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "script_sha256": {Path(p).name: live.sha256(p) for p in (__file__, contracts.__file__, live.__file__)},
              "binary_sha256": {p.name: live.sha256(p) for p in (contracts.NODE, contracts.MINER)}}

    def checkpoint(stage):
        report["stage"] = stage
        (BASE / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"[stage] {stage}", flush=True)

    def mine(node, count=1):
        first = node.height() + 1
        with (BASE / "logs" / f"worker-{node.name}-{first}.log").open("w") as log:
            result = subprocess.run([str(contracts.MINER), "--rpc", f"http://127.0.0.1:{node.rpc_port}",
                "--key-file", str(key), "--threads", "2", "--rpc-timeout", "600", "--blocks", str(count)],
                cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=max(900, count * 180))
        live.require(result.returncode == 0 and node.height() == first + count - 1, "finite mining failed")

    def states(node, opening, expected, label):
        found = {}
        cursor = None
        tip = None
        for _ in range(10):
            page = rpc(node, "walletListObjectStates", [opening["opening_hex"], cursor, 1])
            current = (page["height"], page["tip_hash"])
            live.require(tip is None or tip == current, "tip changed during bounded discovery")
            tip = current
            live.require(len(page["states"]) <= 1, "discovery ignored page limit")
            for item in page["states"]:
                address = item["object"]["address"]
                live.require(address not in found, "discovery cursor repeated an entry")
                found[address] = item["has_balance"]
            cursor = page["next_root"]
            if cursor is None:
                break
        live.require(cursor is None and found == expected, f"{label}: discovered states differ: {found}")
        report["observations"].append({"node": node.name, "label": label, "height": tip[0],
                                       "tip_hash": tip[1], "balances": found})

    def work(node, common):
        total = 0
        for height in range(common + 1, node.height() + 1):
            target = int.from_bytes(bytes.fromhex(rpc(node, "getBlockHeader", [height])["difficulty_target"]), "little")
            live.require(target > 0, "invalid target")
            total += ((1 << 256) - 1) // target + 1
        return total

    try:
        b.start("01-receiver")
        a.start("02-producer", mode="extminer", genesis=True, seeds=[b.seed])
        live.wait_value("copied fixtures converge", lambda: live.exact_tip(a, b), 600)
        live.require(a.info() == prior["final_tip"], "source tip differs")
        owner = rpc(a, "walletActiveAddress")["address"]
        recovery = rpc(b, "walletActiveAddress")["address"]
        definition = {"kind": "custom_program", "definition": {
            "state": ["0", "0"],
            "program": [{"opcode": "add", "destination": "state0", "left": "state0", "right": "one",
                         "predicate": {"source": "terminal", "inverted": True}, "immediate": "0"}],
            "claim_authority": owner, "recovery_authority": recovery,
            "claim_recipient": owner, "recovery_recipient": recovery,
            "deadline_height": 1000000, "max_fee_micronoid": 1000000,
            "max_payout_micronoid": 1000000, "min_retained_micronoid": 0,
            "claim_can_continue": True, "claim_can_close": True,
            "recovery_can_continue": False, "recovery_can_close": True,
            "unrestricted_payout_recipient": False}}
        opening = rpc(a, "createObject", [definition])
        rpc(b, "walletWatchObject", [opening["opening_hex"]])
        checkpoint("fund a shared watched contract and save the common branch")
        funded = rpc(a, "walletFundObject", [opening["opening_hex"], 10000000, 0])
        live.wait_value("funding reached producer mempool", lambda: rpc(a, "getMempoolEntry", [funded["txid"]]) is not None, 120)
        mine(a)
        live.wait_value("both participants accepted funding", lambda: live.exact_tip(a, b), 600)
        common = a.height()
        for node in (a, b):
            states(node, opening, {opening["address"]: True}, "original balance")
        b.stop()
        subprocess.run(["cp", "-a", "--reflink=auto", "--sparse=always", str(BASE / 'receiver'), str(BASE / 'alternate')], check=True)
        b.start("03-receiver-watching", seeds=[a.seed])

        checkpoint("call from one authority and discover its successor from the other wallet")
        slot = rpc(a, "getObjectInstances", [opening["opening_hex"], 0, 1])["slots"][0]
        request = {"opening_hex": opening["opening_hex"], "slot_index": slot["slot_index"],
                   "creation_id": slot["creation_id"], "terminal": False, "fee_micronoid": 0,
                   "expected_authority": owner}
        preview = rpc(a, "previewObjectCall", [request])
        request.update(expected_txid=preview["txid"], expected_call_height=preview["call_height"], expected_recovery=preview["recovery"])
        called = rpc(a, "walletCallObject", [request])
        successor = called["successor"]
        txid = called["transaction"]["txid"]
        states(a, opening, {opening["address"]: True, successor["address"]: False}, "pending is not a balance")
        live.wait_value("call reached producer mempool", lambda: rpc(a, "getMempoolEntry", [txid]) is not None, 120)
        mine(a)
        live.wait_value("other participant accepted the call", lambda: live.exact_tip(a, b), 600)
        for node in (a, b):
            states(node, opening, {opening["address"]: False, successor["address"]: True}, "confirmed successor")
        receipt = rpc(a, "exportObjectReceipt", [opening["opening_hex"], txid])
        a_work = work(a, common)
        report.update(common_height=common, opening=opening, successor=successor, call=called,
                      losing_tip=a.info(), losing_work=str(a_work))
        a.stop()
        b.stop()

        checkpoint("build a strictly heavier branch retaining the original funded balance")
        alternate.start("04-alternate", mode="extminer", genesis=True)
        live.require(alternate.height() == common, "alternate branch did not start at common funding")
        mine(alternate, 2)
        while work(alternate, common) <= a_work:
            live.require(alternate.height() < common + 10, "alternate did not overtake within the bound")
            mine(alternate)
        report.update(winning_tip=alternate.info(), winning_work=str(work(alternate, common)))
        alternate.stop()
        alternate.start("05-winning-peer")
        a.start("06-producer-reorg", seeds=[alternate.seed])
        live.wait_value("watched wallet selects the winning branch", lambda: live.exact_tip(a, alternate), 900)
        states(a, successor, {opening["address"]: True, successor["address"]: False}, "predecessor restored by reorg")
        live.require(rpc(a, "getTx", [txid]) is None, "orphaned call still confirmed")
        report["orphan_receipt_rejected"] = contracts.rejected(a, "verifyObjectReceipt", [receipt])
        live.require("reorg complete" in a.log_path.read_text(errors='replace'), "missing reorg evidence")
        a.stop()
        a.start("07-producer-restart", seeds=[alternate.seed])
        live.wait_value("restarted watched wallet has winning tip", lambda: live.exact_tip(a, alternate), 600)
        states(a, successor, {opening["address"]: True, successor["address"]: False}, "restored discovery after restart")
        report.update(status="passed", final_tip=a.info())
        checkpoint("complete")
    except Exception as error:
        report.update(status="failed", error=str(error))
        checkpoint("failed")
        raise
    finally:
        for node in (a, b, alternate):
            node.request_stop()
        for node in (a, b, alternate):
            try:
                node.finish_stop()
            except Exception as error:
                report.setdefault("shutdown_errors", []).append(str(error))
        (BASE / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()

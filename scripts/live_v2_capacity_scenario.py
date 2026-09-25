#!/usr/bin/env python3
"""Qualify full candidate blocks on a real 4 CPU / 8 GiB P2P receiver.

Run inside a loopback-only namespace after the contract scenario has PASSED
and stopped both nodes. NOID_V2_CAPACITY_SOURCE identifies that source;
NOID_V2_LIVE_DIR must be a fresh destination. The script copies only those
isolated fixtures, preserving the prior report and binary identity.
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
    source = Path(os.environ["NOID_V2_CAPACITY_SOURCE"]).resolve()
    prior = json.loads((source / "report.json").read_text())
    live.require(prior["status"] == "passed" and not prior.get("shutdown_errors"), "source did not pass and shut down")
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
    unit = f"noid-v2-full-receiver-{os.getpid()}"
    b = contracts.Node("receiver", 26310, 26311, command_prefix=("systemd-run", "--user", "--scope", "--quiet",
        f"--unit={unit}", "-p", "MemoryMax=8G", "-p", "MemorySwapMax=0",
        "-p", "CPUQuota=400%", "taskset", "-c", "0,2,4,6"))
    a = contracts.Node("producer", 26300, 26301, command_prefix=("taskset", "-c", "1,3,5,7,8,9,10,11"))
    a.large = True
    report = {"status": "running", "source_tip": prior["final_height"], "blocks": [], "admission": [],
              "source_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "script_sha256": {Path(p).name: live.sha256(p) for p in (__file__, contracts.__file__, live.__file__)},
              "receiver_cpu_affinity": [0, 2, 4, 6], "receiver_backend": "pclmul",
              "receiver_memory_max": 8 * 1024**3, "receiver_swap_max": 0,
              "binary_sha256": {p.name: live.sha256(p) for p in (contracts.NODE, contracts.MINER)}}

    def checkpoint(stage):
        report["stage"] = stage
        (BASE / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"[stage] {stage}", flush=True)

    def resources():
        return contracts.receiver_resources(unit)

    def submit_workload(label, create):
        before = resources()
        started = time.monotonic()
        result = create()
        # Call batches also return their successor opening.
        txids = result[0] if isinstance(result, tuple) else result
        for node in (a, b):
            for txid in txids:
                live.wait_value(f"{node.name} admits the complete workload",
                    lambda node=node, txid=txid: rpc(node, "getMempoolEntry", [txid]) is not None, 180)
        after = resources()
        cpu_before = dict(line.split() for line in before["cpu.stat"].splitlines())
        cpu_after = dict(line.split() for line in after["cpu.stat"].splitlines())
        report["admission"].append({"label": label, "transactions": len(txids),
            "wallet_submission_and_receiver_admission_seconds": time.monotonic() - started,
            "receiver_cpu_seconds": (int(cpu_after["usage_usec"]) - int(cpu_before["usage_usec"])) / 1_000_000,
            "receiver_before": before, "receiver_after": after})
        return result

    def mine(label, txids=(), expected_class="Small", expected_calls=0):
        for txid in txids:
            live.wait_value("producer retains submitted transaction", lambda txid=txid: rpc(a, "getMempoolEntry", [txid]) is not None, 120)
        height = a.height() + 1
        start = time.monotonic()
        offset = a.log_path.stat().st_size
        checkpoint(f"H{height} {label}")
        before = resources()
        with (BASE / "logs" / f"worker-{height}.log").open("w") as log:
            result = subprocess.run([str(contracts.MINER), "--rpc", f"http://127.0.0.1:{a.rpc_port}",
                "--key-file", str(key), "--threads", "2", "--rpc-timeout", "600", "--blocks", "1"],
                cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=900)
        live.require(result.returncode == 0 and a.height() == height, "finite external mining failed")
        live.wait_value("receiver accepted exact full-block tip", lambda: live.exact_tip(a, b), 900)
        details = rpc(a, "getBlockDetails", [height])
        actual = {tx["txid"] for tx in details["retained"]["transactions"]}
        live.require(set(txids).issubset(actual), "not all submitted transactions entered the block")
        user_transactions = [tx for tx in details["retained"]["transactions"] if not tx["coinbase"]]
        appended = a.log_path.read_bytes()[offset:].decode(errors="replace")
        live.require(f"proof_class=V2({expected_class})" in appended, "block used an unexpected proof class")
        report["blocks"].append({"height": height, "label": label, "user_transactions": len(txids),
            "accepted_user_transactions": len(user_transactions),
            "accepted_user_pages": sum(tx["page_count"] for tx in user_transactions),
            "accepted_live_inputs": sum(tx["live_inputs"] for tx in user_transactions),
            "accepted_live_outputs": sum(tx["live_outputs"] for tx in user_transactions),
            "contract_calls": expected_calls, "class": expected_class,
            "production_and_delivery_seconds": time.monotonic() - start,
            "receiver_before": before, "receiver_after": resources()})

    def split_self(count):
        result = []
        for _ in range(count):
            available = [u["value_micronoid"] for u in rpc(a, "walletListUtxos") if not u["reserved"] and u["value_micronoid"] > 40000]
            live.require(available, "no splittable wallet input")
            fee = 20000
            amount = (max(available) - fee) // 2
            # Avoid an exact-change one-output send while preserving one input.
            while amount + fee in available:
                amount -= 1
            sent = rpc(a, "walletSend", [payer, amount, fee])
            live.require(sent["input_count"] == 1 and sent["output_count"] == 2, "split was not one-input/two-output")
            result.append(sent["txid"])
        return result

    def input_payment(count):
        available = sorted((u["value_micronoid"] for u in rpc(a, "walletListUtxos")
                            if not u["reserved"]), reverse=True)
        live.require(len(available) >= count, "not enough independent inputs")
        fee = 100000
        amount = sum(available[:count]) - fee
        live.require(amount > 0, "input-boundary payment has no value")
        plan = rpc(a, "walletPlanSend", [payer, amount, fee])
        live.require(plan["input_count"] == count and plan["output_count"] == 1,
                     "wallet selected a different input-boundary layout")
        sent = rpc(a, "walletSend", [payer, amount, fee])
        live.require(sent["input_count"] == count and sent["output_count"] == 1,
                     "submitted input-boundary payment differs from its plan")
        return sent["txid"]

    try:
        b.start("01-receiver")
        report["receiver_enforcement"] = resources()
        a.start("02-producer", mode="extminer", genesis=True, seeds=[b.seed])
        live.wait_value("copied fixtures converge", lambda: live.exact_tip(a, b), 600)
        live.require(a.height() == prior["final_height"], "copied fixture tip differs")
        payer = rpc(a, "walletActiveAddress")["address"]
        protocol = rpc(a, "getContractProtocol")
        report["protocol"] = protocol
        limits = {entry["class"]: entry for entry in protocol["classes"]}
        small, large = limits["small"], limits["large"]
        live.require(small["pages"] == small["contract_calls"] == 63,
                     "this scenario requires the 63-page/call standard class")
        live.require(small["live_inputs"] == large["live_inputs"] == 504
                     and large["contract_calls"] == 63,
                     "classes must share the selected input and call limits")
        small_pages, large_pages = small["pages"], large["pages"]
        live.require(small_pages < large_pages <= large["live_inputs"],
                     "Large must fit more one-input payments than Small")
        # Leave room for both full-call blocks and the receipt follow-up run.
        funding_amount, funding_fee = 200000, 20000
        minimum_input = funding_amount + funding_fee
        while len([u for u in rpc(a, "walletListUtxos") if not u["reserved"] and u["value_micronoid"] > minimum_input]) < 64:
            ready = len([u for u in rpc(a, "walletListUtxos") if not u["reserved"] and u["value_micronoid"] > minimum_input])
            live.require(ready > 0, "fixture has no spendable inventory")
            mine("prepare wallet inventory", split_self(min(ready, 32)))
        definition = {"kind": "custom_program", "definition": {
            "state": ["0", "0"],
            "program": [{"opcode": "add", "destination": "state0", "left": "state0", "right": "one",
                         "predicate": {"source": "terminal", "inverted": True}, "immediate": "0"}],
            "claim_authority": payer, "recovery_authority": payer,
            "claim_recipient": payer, "recovery_recipient": payer,
            "deadline_height": 1000000, "max_fee_micronoid": 1000000,
            "max_payout_micronoid": 1000000, "min_retained_micronoid": 0,
            "claim_can_continue": True, "claim_can_close": True,
            "recovery_can_continue": False, "recovery_can_close": True,
            "unrestricted_payout_recipient": False}}
        opening = rpc(a, "createObject", [definition])
        rpc(b, "walletWatchObject", [opening["opening_hex"]])
        funding = submit_workload("full Small ordinary payments", lambda: [
            rpc(a, "walletFundObject", [opening["opening_hex"], funding_amount, funding_fee])["txid"] for _ in range(63)])
        mine("full 63 ordinary payments", funding)
        instances = rpc(a, "getObjectInstances", [opening["opening_hex"], 0, 64])
        live.require(len(instances["slots"]) == 63, "63 independent deposits not present")
        def call_counters(current):
            calls, successor = [], None
            slots = rpc(a, "getObjectInstances", [current["opening_hex"], 0, 64])["slots"]
            live.require(len(slots) == small["contract_calls"], "missing counter instances")
            for slot in slots:
                request = {"opening_hex": current["opening_hex"], "slot_index": slot["slot_index"],
                    "creation_id": slot["creation_id"], "terminal": False, "fee_micronoid": 20000,
                    "expected_authority": payer}
                preview = rpc(a, "previewObjectCall", [request])
                request.update(expected_txid=preview["txid"], expected_call_height=preview["call_height"],
                               expected_recovery=preview["recovery"])
                called = rpc(a, "walletCallObject", [request])
                calls.append(called["transaction"]["txid"])
                successor = called["successor"]
            return calls, successor

        calls, successor = submit_workload("full Small contract calls", lambda: call_counters(opening))
        mine("full 63 persistent-counter calls with Large enabled", calls, expected_calls=63)
        live.require(successor["state"] == ["1", "0"], "wrong persistent successor")
        live.require(len(rpc(b, "getObjectInstances", [successor["opening_hex"], 0, 64])["slots"]) == 63, "receiver lost contract instances")
        for txid in (calls[0], calls[-1]):
            receipt = rpc(b, "exportObjectReceipt", [opening["opening_hex"], txid])
            live.require(rpc(b, "verifyObjectReceipt", [receipt])["valid"], "full-prefix receipt failed")
        while len([u for u in rpc(a, "walletListUtxos") if not u["reserved"] and u["value_micronoid"] > 40000]) < large_pages + 1:
            mine("prepare Large payment inputs", split_self(small_pages))
        payments = submit_workload("full Large ordinary payments", lambda: split_self(large_pages))
        mine(f"full {large_pages} ordinary payments", payments, expected_class="Large")
        def mixed_workload():
            calls, mixed_successor = call_counters(successor)
            return calls + split_self(large_pages - len(calls)), mixed_successor
        mixed, mixed_successor = submit_workload("full Large mixed block", mixed_workload)
        calls, payments = mixed[:63], mixed[63:]
        mine(f"full Large: {len(calls)} calls and {len(payments)} payments",
             calls + payments, expected_class="Large", expected_calls=len(calls))
        live.require(mixed_successor["state"] == ["2", "0"], "wrong mixed-block successor")
        live.require(len(rpc(b, "getObjectInstances", [mixed_successor["opening_hex"], 0, 64])["slots"]) == 63,
                     "receiver lost mixed-block contract instances")
        for txid in (calls[0], calls[-1]):
            receipt = rpc(b, "exportObjectReceipt", [successor["opening_hex"], txid])
            live.require(rpc(b, "verifyObjectReceipt", [receipt])["valid"], "mixed Large receipt failed")
        report["counter_opening"] = mixed_successor

        # One multi-page spend plus single-input payments reaches both limits:
        # at P206/I504, 341 inputs use 43 pages and 163 more payments fill them.
        combined_inputs = next(count for count in range(1, large["live_inputs"] + 1)
            if (count + 7) // 8 + large["live_inputs"] - count == large_pages)
        additional_payments = large["live_inputs"] - combined_inputs
        while len([u for u in rpc(a, "walletListUtxos")
                   if not u["reserved"] and u["value_micronoid"] > 40000]) < large["live_inputs"] + 1:
            mine("prepare maximum-input inventory", split_self(small_pages))
        maximum = submit_workload("Large reaches both page and input limits",
            lambda: [input_payment(combined_inputs)] + split_self(additional_payments))
        mine(f"Large: {large_pages} pages and {large['live_inputs']} inputs",
             maximum, expected_class="Large")
        live.require(report["blocks"][-1]["accepted_user_pages"] == large_pages
                     and report["blocks"][-1]["accepted_live_inputs"] == large["live_inputs"],
                     "accepted Large block did not reach both limits")
        report["large_input_boundary"] = {"pages": large_pages,
            "inputs": large["live_inputs"], "logical_transactions": len(maximum),
            "multi_page_transaction_inputs": combined_inputs,
            "additional_one_input_payments": additional_payments, "height": a.height()}
        mine("full Small immediately after Large", split_self(small_pages))

        while len([u for u in rpc(a, "walletListUtxos")
                   if not u["reserved"] and u["value_micronoid"] > 40000]) < small["live_inputs"] + 1:
            mine("prepare Small maximum-input inventory", split_self(small_pages))
        available = sorted((u["value_micronoid"] for u in rpc(a, "walletListUtxos")
                            if not u["reserved"]), reverse=True)
        fee = 100000
        over_limit = sum(available[:small["live_inputs"] + 1]) - fee
        negative = {}
        for method in ("walletPlanSend", "walletSend"):
            rejected = contracts.rejected(a, method, [payer, over_limit, fee])
            live.require("InputLimitExceeded" in rejected and "'max_inputs': 504" in rejected,
                         "505 inputs did not receive the stable active-budget error")
            negative[method] = rejected
        maximum = submit_workload("Small maximum-input payment",
            lambda: [input_payment(small["live_inputs"])])
        mine(f"Small: {small['live_inputs']} inputs in one paged spend", maximum)
        live.require(report["blocks"][-1]["accepted_user_pages"] == small_pages
                     and report["blocks"][-1]["accepted_live_inputs"] == small["live_inputs"],
                     "accepted Small block did not reach its input limit")
        report["small_input_boundary"] = {"pages": (small["live_inputs"] + 7) // 8,
            "inputs": small["live_inputs"], "logical_transactions": 1,
            "height": a.height(), "over_limit_rejections": negative}
        for index in range(8):
            mine(f"Small carried-Large tail {index + 1}/8")
        report["receiver_final"] = resources()
        b.stop()
        b.start("03-receiver-restart", seeds=[a.seed])
        live.wait_value("receiver reopens after full and alternating blocks", lambda: live.exact_tip(a, b), 600)
        report.update(status="passed", final_tip=a.info())
        checkpoint("complete")
    except Exception as error:
        report.update(status="failed", error=str(error))
        checkpoint("failed")
        raise
    finally:
        for node in (a, b):
            node.request_stop()
        for node in (a, b):
            try:
                node.finish_stop()
            except Exception as error:
                report.setdefault("shutdown_errors", []).append(str(error))
        (BASE / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()

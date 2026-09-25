#!/usr/bin/env python3
"""Check shared receipt storage using a completed full-capacity fixture.

Run in a loopback-only namespace after the source nodes have stopped.
NOID_V2_CAPACITY_SOURCE points to the passed capacity scenario and
NOID_V2_LIVE_DIR to a fresh directory. Requires a rebuilt isolated daemon.
NOID_V2_PRUNED_RECEIPTS_SOURCE optionally checks old retained receipts from
the ancestor contract scenario after its original block bodies were pruned.
"""
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess

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
    live.require(prior["status"] == "passed" and not prior.get("shutdown_errors"), "source qualification did not pass and stop")
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
    unit = f"noid-v2-receipts-receiver-{os.getpid()}"
    b = contracts.Node("receiver", 26310, 26311, command_prefix=("systemd-run", "--user", "--scope", "--quiet",
        f"--unit={unit}", "-p", "MemoryMax=8G", "-p", "MemorySwapMax=0",
        "-p", "CPUQuota=400%", "taskset", "-c", "0,2,4,6"))
    a = contracts.Node("producer", 26300, 26301, command_prefix=("taskset", "-c", "1,3,5,7,8,9,10,11"))
    report = {"status": "running", "source_tip": prior["final_tip"], "receipts": [],
              "source_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "script_sha256": {Path(p).name: live.sha256(p) for p in (__file__, contracts.__file__, live.__file__)},
              "receiver_cpu_affinity": [0, 2, 4, 6], "receiver_backend": "pclmul",
              "receiver_memory_max": 8 * 1024**3, "receiver_swap_max": 0,
              "binary_sha256": {p.name: live.sha256(p) for p in (contracts.NODE, contracts.MINER)}}

    def checkpoint(stage):
        report["stage"] = stage
        (BASE / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"[stage] {stage}", flush=True)

    def mine():
        height = a.height() + 1
        with (BASE / "logs" / f"worker-{height}.log").open("w") as log:
            result = subprocess.run([str(contracts.MINER), "--rpc", f"http://127.0.0.1:{a.rpc_port}",
                "--key-file", str(key), "--threads", "2", "--rpc-timeout", "600", "--blocks", "1"],
                cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=900)
        live.require(result.returncode == 0 and a.height() == height, "finite mining failed")
        live.wait_value("receipt nodes accept identical tip", lambda: live.exact_tip(a, b), 600)

    def old_receipts():
        path = os.environ.get("NOID_V2_PRUNED_RECEIPTS_SOURCE")
        if path is None:
            return []
        ancestor = json.loads((Path(path) / "report.json").read_text())
        live.require(ancestor["status"] == "passed" and ancestor.get("pruning") == "passed", "ancestor pruning qualification did not pass")
        checks = []
        # Include a direct receipt and the offline receiver's descendant form.
        for record in (ancestor["calls"][0], ancestor["calls"][-1]):
            txid = record["result"]["transaction"]["txid"]
            for node in (a, b):
                encoded = rpc(node, "exportObjectReceipt", [record["opening_hex"], txid])
                local = (node.data_dir / 'objects' / (txid + '.receipt')).read_bytes()
                live.require(local[:8] in (b'O1OBJRC4', b'O1OBJRC5', b'NOIDORF1'), "unknown retained receipt format")
                if local[:8] != b'NOIDORF1':
                    live.require(local.hex() == encoded, "reading changed an old complete receipt")
                live.require(rpc(node, "verifyObjectReceipt", [encoded])["valid"], "old pruned receipt failed with new binary")
                checks.append({"node": node.name, "txid": txid, "format": local[:8].decode(),
                               "bytes": len(local), "sha256": hashlib.sha256(local).hexdigest()})
        return checks

    try:
        b.start("01-receiver")
        report["receiver_enforcement"] = contracts.receiver_resources(unit)
        a.start("02-producer", mode="extminer", genesis=True, seeds=[b.seed])
        live.wait_value("copied full-capacity nodes converge", lambda: live.exact_tip(a, b), 600)
        live.require(a.info() == prior["final_tip"], "copied fixture tip differs")
        checkpoint("verify original pruned receipts without rewriting them")
        report["old_receipts"] = old_receipts()
        payer = rpc(a, "walletActiveAddress")["address"]
        definition = {"kind": "custom_program", "definition": {
            "state": ["1", "0"],
            "program": [{"opcode": "add", "destination": "state0", "left": "state0", "right": "one",
                         "predicate": {"source": "terminal", "inverted": True}, "immediate": "0"}],
            "claim_authority": payer, "recovery_authority": payer,
            "claim_recipient": payer, "recovery_recipient": payer,
            "deadline_height": 1000000, "max_fee_micronoid": 1000000,
            "max_payout_micronoid": 1000000, "min_retained_micronoid": 0,
            "claim_can_continue": True, "claim_can_close": True,
            "recovery_can_continue": False, "recovery_can_close": True,
            "unrestricted_payout_recipient": False}}
        opening = prior.get("counter_opening") or rpc(a, "createObject", [definition])
        counter_state = int(opening["state"][0])
        live.require(counter_state in (1, 2), "unexpected capacity-fixture counter state")
        rpc(b, "walletWatchObject", [opening["opening_hex"]])
        instances = rpc(a, "getObjectInstances", [opening["opening_hex"], 0, 64])["slots"]
        live.require(len(instances) == 63, "source does not have 63 expected counters")
        checkpoint("authorize 63 independent counter updates")
        calls = []
        for slot in instances:
            request = {"opening_hex": opening["opening_hex"], "slot_index": slot["slot_index"],
                "creation_id": slot["creation_id"], "terminal": False, "fee_micronoid": 0,
                "expected_authority": payer}
            preview = rpc(a, "previewObjectCall", [request])
            request.update(expected_txid=preview["txid"], expected_call_height=preview["call_height"],
                           expected_recovery=preview["recovery"])
            submitted = rpc(a, "walletCallObject", [request])
            live.require(submitted["successor"]["state"] == [str(counter_state + 1), "0"],
                         "shared-receipt call did not increment the fixture counter")
            calls.append(submitted["transaction"]["txid"])
            report["counter_opening"] = submitted["successor"]
        for txid in calls:
            live.wait_value("producer retains reviewed counter call",
                lambda txid=txid: rpc(a, "getMempoolEntry", [txid]) is not None, 120)
        checkpoint("mine the full contract block with shared local receipts")
        mine()
        report["call_height"] = a.height()
        checkpoint("reconstruct every portable receipt and inspect local proof sharing")
        report["storage"] = []
        for node in (a, b):
            terminal_names = set()
            reference_bytes = 0
            portable_bytes = 0
            for txid in calls:
                encoded = rpc(node, "exportObjectReceipt", [opening["opening_hex"], txid])
                portable = bytes.fromhex(encoded)
                live.require(portable[:8] in (b'O1OBJRC4', b'O1OBJRC5'), "export is not a complete receipt")
                local = (node.data_dir / 'objects' / (txid + '.receipt')).read_bytes()
                live.require(local.startswith(b'NOIDORF1'), "local receipt did not use shared proof storage")
                terminal_names.add(local[8:40].hex() + '.terminal')
                reference_bytes += len(local)
                portable_bytes += len(portable)
                report["receipts"].append({"node": node.name, "txid": txid,
                    "sha256": hashlib.sha256(portable).hexdigest(), "bytes": len(portable)})
                if txid in (calls[0], calls[-1]):
                    live.require(rpc(node, "verifyObjectReceipt", [encoded])["valid"], "exported recursive receipt failed")
            live.require(len(terminal_names) == 1, "one block stored multiple distinct shared proofs")
            proof_bytes = (node.data_dir / 'objects' / 'terminals' / next(iter(terminal_names))).stat().st_size
            live.require(reference_bytes + proof_bytes < portable_bytes // 20, "shared receipt storage did not reduce duplication")
            report["storage"].append({"node": node.name, "calls": len(calls), "reference_bytes": reference_bytes,
                "shared_proof_bytes": proof_bytes, "stored_bytes": reference_bytes + proof_bytes,
                "separate_portable_receipts_bytes": portable_bytes})
        checkpoint("restart and export byte-identical receipts")
        mine()  # Exercises the retained-window reader on the new local format.
        report["receiver_before_restart"] = contracts.receiver_resources(unit)
        b.stop()
        b.start("03-receiver-restart", seeds=[a.seed])
        live.wait_value("restarted receiver has the exact tip", lambda: live.exact_tip(a, b), 600)
        for record in report["receipts"]:
            if record["node"] != b.name:
                continue
            encoded = rpc(b, "exportObjectReceipt", [opening["opening_hex"], record["txid"]])
            live.require(hashlib.sha256(bytes.fromhex(encoded)).hexdigest() == record["sha256"], "receipt changed after restart")
        for txid in (calls[0], calls[-1]):
            encoded = rpc(b, "exportObjectReceipt", [opening["opening_hex"], txid])
            live.require(rpc(b, "verifyObjectReceipt", [encoded])["valid"], "restarted receipt failed verification")
        report.update(status="passed", final_tip=a.info(), receiver_final=contracts.receiver_resources(unit))
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

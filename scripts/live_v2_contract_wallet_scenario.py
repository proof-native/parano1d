#!/usr/bin/env python3
"""Two real wallets: GUI files/journal, other-party calls and pruned recovery.

Requires a passed, stopped shared-receipt fixture in NOID_V2_WALLET_SOURCE,
a fresh NOID_V2_LIVE_DIR, and the noid_gui test binary in NOID_GUI_TEST_BINARY.
Run explicitly in a loopback-only network namespace. No CI integration.
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
    live.require(devices == ["lo"], "use a loopback-only network namespace")
    subprocess.run(["ip", "link", "set", "lo", "up"], check=True)
    source = Path(os.environ["NOID_V2_WALLET_SOURCE"]).resolve()
    gui_test = Path(os.environ["NOID_GUI_TEST_BINARY"]).resolve()
    prior = json.loads((source / "report.json").read_text())
    live.require(prior["status"] == "passed" and not prior.get("shutdown_errors"), "source did not pass and stop")
    live.require(not BASE.exists(), "use a fresh output directory")
    BASE.mkdir(parents=True)
    for folder in ("logs", "files", "gui-cases"):
        (BASE / folder).mkdir()
    for old, new in (("producer", "alice"), ("receiver", "bob")):
        subprocess.run(["cp", "-a", "--reflink=auto", "--sparse=always", str(source / old), str(BASE / new)], check=True)
    key = BASE / "mining.key"
    with key.open("x") as file:
        file.write(secrets.token_hex(32) + "\n")
    key.chmod(0o600)
    live.BASE = BASE
    alice = contracts.Node("alice", 26300, 26301)
    bob = contracts.Node("bob", 26310, 26311)
    report = {"status": "running", "source_tip": prior["final_tip"], "gui_checks": [], "blocks": [],
              "binary_sha256": {path.name: live.sha256(path) for path in (contracts.NODE, contracts.MINER, gui_test)},
              "script_sha256": {Path(path).name: live.sha256(path) for path in (__file__, contracts.__file__, live.__file__)}}

    def checkpoint(stage):
        report["stage"] = stage
        (BASE / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print("[stage] " + stage, flush=True)

    def gui(node, action, **values):
        number = len(report["gui_checks"]) + 1
        label = f"{number:02}-{node.name}-{action}"
        case_path = BASE / "gui-cases" / (label + ".json")
        result_path = BASE / "gui-cases" / (label + "-result.json")
        case = dict(values, action=action, rpc_url=f"http://127.0.0.1:{node.rpc_port}",
                    data_dir=str(BASE / (node.name + "-gui")), result_path=str(result_path))
        case_path.write_text(json.dumps(case, indent=2) + "\n")
        env = dict(os.environ, NOID_CONTRACT_WORKFLOW_CASE=str(case_path))
        start = time.monotonic()
        with (BASE / "logs" / (label + ".log")).open("w") as log:
            result = subprocess.run([str(gui_test), "--exact",
                "backend::contracts::workflow::live::file_workflow_on_isolated_nodes", "--ignored"],
                cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=900)
        live.require(result.returncode == 0 and result_path.is_file(), "GUI backend failed: " + label)
        value = json.loads(result_path.read_text())
        report["gui_checks"].append(dict(label=label, seconds=time.monotonic()-start, result=value))
        checkpoint(label)
        return value

    def converge():
        if alice.proc is not None and alice.proc.poll() is None:
            live.wait_value("wallets select the exact same tip", lambda: live.exact_tip(alice, bob), 900)

    def mine(count=1, txid=None):
        if txid:
            live.wait_value("transaction reached producer", lambda: rpc(bob, "getMempoolEntry", [txid]) is not None, 120)
        first = bob.height() + 1
        start = time.monotonic()
        checkpoint(f"produce blocks {first}..{first+count-1}")
        with (BASE / "logs" / f"miner-{first}.log").open("w") as log:
            result = subprocess.run([str(contracts.MINER), "--rpc", f"http://127.0.0.1:{bob.rpc_port}",
                "--key-file", str(key), "--threads", "2", "--rpc-timeout", "600", "--blocks", str(count)],
                cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=max(900, count * 180))
        live.require(result.returncode == 0 and bob.height() == first + count - 1, "finite mining failed")
        converge()
        report["blocks"].append(dict(first=first, last=bob.height(), seconds=time.monotonic()-start))

    def journal(node, opening, expected):
        result = gui(node, "activity", opening_hex=opening["opening_hex"])
        entries = result["operations"]
        live.require({op["txid"] for op in entries} == set(expected) and len(entries) == len(expected), "journal lost or duplicated records")
        live.require(all(op["canonical"] and op["receipt_available"] for op in entries), "missing confirmed receipt")
        return entries

    try:
        bob.start("01-bob-producer", mode="extminer", genesis=True)
        alice.start("02-alice", seeds=[bob.seed])
        converge()
        payer = rpc(alice, "walletActiveAddress")["address"]
        payee = rpc(bob, "walletActiveAddress")["address"]
        live.require(payer != payee, "two different wallet authorities required")
        height = alice.height()
        definition = {"kind": "custom_program", "definition": {
            "state": ["0", "0"], "program": [{"opcode": "add", "destination": "state0",
                "left": "state0", "right": "one", "predicate": {"source": "terminal", "inverted": True}, "immediate": "0"}],
            "claim_authority": payee, "recovery_authority": payer,
            "claim_recipient": payee, "recovery_recipient": payer,
            "deadline_height": height + 10, "max_fee_micronoid": 1000000,
            "max_payout_micronoid": 1000000, "min_retained_micronoid": 0,
            "claim_can_continue": True, "claim_can_close": True,
            "recovery_can_continue": False, "recovery_can_close": True,
            "unrestricted_payout_recipient": False}}
        funded = gui(alice, "create_and_fund", definition=definition, name="Alice’s shared budget", amount=10000000)
        original = funded["info"]
        mine(txid=funded["txid"])
        shared = BASE / "files" / "Общий контракт 東京.json"
        live.require(gui(alice, "share", opening_hex=original["opening_hex"], path=str(shared))["with_receipt"], "funding receipt missing from shared terms")
        gui(bob, "import", path=str(shared))
        journal(bob, original, [funded["txid"]])
        first = gui(bob, "call", opening_hex=original["opening_hex"], terminal=False,
                    payout={"address": payee, "amount_micronoid": 1000000})
        mine(txid=first["txid"])
        state1 = first["successor"]
        entries = journal(alice, original, [funded["txid"], first["txid"]])
        other = next(op for op in entries if op["txid"] == first["txid"])
        live.require(other["authority"] == payee and other["amount_micronoid"] == 1000000, "other-party facts differ")

        checkpoint("Alice offline; Bob makes a second call")
        alice.stop()
        second = gui(bob, "call", opening_hex=state1["opening_hex"], terminal=False,
                     payout={"address": payee, "amount_micronoid": 1000000})
        mine(txid=second["txid"])
        second_height = bob.height()
        state2 = second["successor"]
        live.require(state2["state"][0] == "2", "counter did not advance twice")
        update = BASE / "files" / "Updated contract.json"
        live.require(gui(bob, "share", opening_hex=state2["opening_hex"], path=str(update))["with_receipt"], "successor file has no call proof")
        # Native local serving retention is 42 blocks. Advance past it without
        # special pruning flags or deleting chain files by hand.
        mine(43)
        live.require(rpc(bob, "getBlockDetails", [second_height])["retained"] is None, "old call body is still retained")
        live.require(rpc(bob, "getTx", [second["txid"]]) is None, "old transaction is still served")
        checkpoint("Alice returns after pruning and imports Bob’s verified update")
        alice.start("03-alice-after-pruning", seeds=[bob.seed])
        converge()
        journal(alice, original, [funded["txid"], first["txid"]])
        known = rpc(alice, "walletListObjectStates", [original["opening_hex"], None, 64])
        live.require(not any(item["object"]["address"] == state2["address"] for item in known["states"]), "missed successor was unexpectedly already known")
        imported = gui(alice, "import", path=str(update))
        live.require(imported["library"][0]["info"]["address"] == state2["address"], "fresh live successor not selected")
        expected = [funded["txid"], first["txid"], second["txid"]]
        journal(alice, original, expected)
        gui(alice, "import", path=str(update))
        journal(alice, original, expected)
        # An older terms/funding file must neither erase the new call nor make
        # the wallet select its spent predecessor again.
        older = gui(alice, "import", path=str(shared))
        live.require(older["library"][0]["info"]["address"] == state2["address"], "stale import replaced live counters")
        live.require(older["library"][0]["name"] == "Alice’s shared budget", "local name changed")
        journal(alice, original, expected)

        # A receipt bound to another exact opening must fail before retention.
        encoded = json.loads(update.read_text())["proof"]["receipt_hex"]
        wrong = contracts.rejected(alice, "walletImportObjectReceipt", [encoded, original["opening_hex"]])
        bad = bytearray.fromhex(encoded); bad[-1] ^= 1
        corrupt = contracts.rejected(alice, "walletImportObjectReceipt", [bad.hex(), None])
        report["negative_cases"] = dict(wrong_opening=wrong, damaged_proof=corrupt)

        checkpoint("Alice recovers; Bob imports the closing receipt with his records intact")
        closed = gui(alice, "call", opening_hex=state2["opening_hex"], terminal=True, payout=None)
        mine(txid=closed["txid"])
        closing = BASE / "files" / "Closing call.receipt"
        gui(alice, "export_receipt", opening_hex=original["opening_hex"], txid=closed["txid"], path=str(closing))
        gui(bob, "import", path=str(closing))
        expected.append(closed["txid"])
        journal(bob, original, expected)
        gui(bob, "import", path=str(closing))
        journal(bob, original, expected)
        bob.stop()
        bob.start("05-bob-restart", seeds=[alice.seed])
        converge()
        journal(bob, original, expected)
        # Export the call that Alice missed using her retained imported proof,
        # after the chain's ordinary transaction lookup has disappeared.
        recovered = BASE / "files" / "Recovered after pruning.receipt"
        gui(alice, "export_receipt", opening_hex=original["opening_hex"], txid=second["txid"], path=str(recovered))
        check = rpc(alice, "verifyObjectReceipt", [recovered.read_bytes().hex()])
        live.require(check["valid"] and check["txid"] == second["txid"], "recovered proof did not verify")
        report.update(status="passed", final_tip=alice.info(), pruning="passed", gui_rendering_tested=False,
                      ordinary_lookup_pruned=True, independent_wallets=True)
        checkpoint("complete")
    except Exception as error:
        report.update(status="failed", error=str(error))
        checkpoint("failed")
        raise
    finally:
        for node in (alice, bob):
            node.request_stop()
        for node in (alice, bob):
            try:
                node.finish_stop()
            except Exception as error:
                report.setdefault("shutdown_errors", []).append(str(error))
        (BASE / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Exercise the scheduled integer-contract fork through real isolated daemons.

Build the separately named isolated_v2_node with pinned candidate material,
parano1d-cli, and parano1d-miner. Run this script in a loopback-only network
namespace. The receiver is limited to four logical CPUs and 8 GiB with no swap.
Every new block, wallet authorization, receipt and network acceptance uses the
normal production path. This is qualification of a candidate, not a release pin.
"""
import json
import os
from pathlib import Path
import secrets
import subprocess
import time

import live_two_miner_fork_reorg_scenario as live

ROOT = Path(__file__).resolve().parents[1]
BASE = Path(os.environ.get("NOID_V2_LIVE_DIR", ROOT / "target/live-tests/v2-integer-contracts"))
NODE = ROOT / "target/release/examples/isolated_v2_node"
MINER = ROOT / "target/release/parano1d-miner"
CLI = ROOT / "target/release/parano1d-cli"


class Node(live.Node):
    def spawn(self, label, mode="node", genesis=False, seeds=None):
        live.require(self.proc is None or self.proc.poll() is not None, "node already running")
        self.root.mkdir(parents=True, exist_ok=True)
        self.log_path = BASE / "logs" / (label + ".log")
        self.log_path.parent.mkdir(parents=True, exist_ok=True)
        args = [*self.command_prefix, str(NODE), "--mode", mode, "--config", str(self.config),
                "--data-dir", str(self.data_dir), "--disable-dns-seeds",
                "--p2p-listen", self.seed, "--rpc-listen", f"127.0.0.1:{self.rpc_port}",
                "--log", "info"]
        if genesis: args += ["--genesis"]
        if mode == "extminer": args += ["--mining-key-file", str(BASE / "mining.key")]
        if getattr(self, "large", False): args += ["--v2-large-blocks"]
        for seed in seeds or []: args += ["--seed", seed]
        self.log_handle = open(self.log_path, "wb", buffering=0)
        started = time.monotonic()
        environment = os.environ.copy()
        if self.name == "receiver":
            # Conservative seed proxy: do not credit this laptop's VPCLMUL.
            environment["NOID_CPU_BACKEND"] = "pclmul"
        self.proc = subprocess.Popen(args, cwd=ROOT, env=environment,
                                     stdout=self.log_handle, stderr=subprocess.STDOUT)
        self.stopping = False
        print(f"[start] {label} pid={self.proc.pid}", flush=True)
        return started


def rpc(node, method, params=None):
    return live.rpc(node.rpc_port, method, params, timeout=600)


def receiver_resources(unit):
    group = subprocess.check_output(["systemctl", "--user", "show", unit + ".scope",
        "-p", "ControlGroup", "--value"], text=True).strip()
    live.require(group, "receiver resource scope is missing")
    control = Path("/sys/fs/cgroup") / group.lstrip("/")
    values = {name: (control / name).read_text().strip() for name in (
        "memory.max", "memory.swap.max", "cpu.max", "memory.current", "memory.peak",
        "memory.events", "cpu.stat")}
    live.require(values["memory.max"] == str(8 * 1024**3)
                 and values["memory.swap.max"] == "0", "receiver memory limits differ")
    quota, period = map(int, values["cpu.max"].split())
    live.require(quota == 4 * period, "receiver CPU quota differs")
    return values


def cli(node, arguments):
    result = subprocess.run([str(CLI), "--rpc", f"http://127.0.0.1:{node.rpc_port}",
        "--rpc-timeout", "600", "--json", "contract", *map(str, arguments)],
        cwd=ROOT, capture_output=True, text=True, timeout=900)
    live.require(result.returncode == 0, f"CLI {arguments[0]} failed: {result.stderr}\n{result.stdout}")
    return json.loads(result.stdout)


def rejected(node, method, params):
    try: rpc(node, method, params)
    except live.LiveForkReorgError as error:
        live.require("transport failed" not in str(error), str(error))
        return str(error)
    raise live.LiveForkReorgError(f"forbidden {method} accepted")


def output_for(node, txid, address):
    tx = rpc(node, "getTx", [txid])
    if tx is None: return None
    details = rpc(node, "getBlockDetails", [tx["height"]])
    for transaction in details["retained"]["transactions"]:
        if transaction["txid"] == txid:
            return next((o for o in transaction["outputs"] if o["owner"] == address), None)
    return None


def main():
    devices = [line.split(":")[0].strip() for line in Path("/proc/net/dev").read_text().splitlines()[2:]]
    live.require(devices == ["lo"], "use an isolated loopback-only network namespace")
    subprocess.run(["ip", "link", "set", "lo", "up"], check=True)
    live.require(not BASE.exists(), f"fresh directory required: {BASE}")
    for binary in (NODE, MINER, CLI): live.require(binary.is_file(), f"missing binary: {binary}")
    BASE.mkdir(parents=True)
    (BASE / "logs").mkdir()
    artifacts = BASE / "artifacts"; artifacts.mkdir()
    key = BASE / "mining.key"
    with key.open("x") as file: file.write(secrets.token_hex(32) + "\n")
    key.chmod(0o600)
    live.BASE = BASE
    unit = f"noid-v2-contract-receiver-{os.getpid()}"
    b = Node("receiver", 26310, 26311, command_prefix=("systemd-run", "--user", "--scope", "--quiet",
        f"--unit={unit}", "-p", "MemoryMax=8G", "-p", "MemorySwapMax=0",
        "-p", "CPUQuota=400%", "taskset", "-c", "0,2,4,6"))
    a = Node("producer", 26300, 26301, command_prefix=("taskset", "-c", "1,3,5,7,8,9,10,11"))
    report = {"status":"running", "binary_sha256":{p.name:live.sha256(p) for p in (NODE, MINER, CLI)},
        "receiver_cpu_affinity":[0,2,4,6], "receiver_backend":"pclmul", "receiver_memory_max":8*1024**3,
        "negative_cases":{}, "blocks":[], "calls":[]}
    def checkpoint(stage):
        report["stage"] = stage
        (BASE / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"[stage] {stage}", flush=True)
    def converge():
        if b.proc is not None and b.proc.poll() is None:
            live.wait_value("receiver accepts exact tip", lambda: live.exact_tip(a, b), 600)
    def mine(count=1):
        for record in calls:
            txid = record["result"]["transaction"]["txid"]
            live.wait_value("producer received pending contract call",
                lambda txid=txid: rpc(a, "getMempoolEntry", [txid]) is not None, 120)
        before = a.height(); start=time.monotonic()
        checkpoint(f"mine {before+1}..{before+count}")
        with (BASE / "logs" / f"external-{before+1}-{before+count}.log").open("w") as log:
            result = subprocess.run([str(MINER), "--rpc", f"http://127.0.0.1:{a.rpc_port}",
                "--key-file", str(key), "--threads", "2", "--rpc-timeout", "600", "--blocks", str(count)],
                cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=max(900, count*180))
        live.require(result.returncode == 0 and a.height() == before+count, "external miner did not produce the requested blocks")
        converge()
        report["blocks"].append({"first":before+1,"last":before+count,"production_and_delivery_seconds":time.monotonic()-start})
    objects={}; calls=[]
    def request(name, **changes):
        obj=objects[name];slot=obj["slot"]
        value={"opening_hex":obj["info"]["opening_hex"], "slot_index":slot["slot_index"],
            "creation_id":slot["creation_id"], "terminal":False,"payout":None,"fee_micronoid":0}
        value.update(changes);return value
    def call(node, name, close=False, pay=None):
        obj=objects[name]
        payload=request(name, terminal=close, payout=None if pay is None else {"address":payee,"amount_micronoid":pay})
        payload["expected_authority"]=rpc(node,"walletActiveAddress")["address"]
        preview=rpc(node,"previewObjectCall",[payload])
        payload.update(expected_txid=preview["txid"],expected_call_height=preview["call_height"],expected_recovery=preview["recovery"])
        result=rpc(node,"walletCallObject",[payload])
        live.require(result["transaction"]["txid"] == preview["txid"] and result["successor"] == preview["successor"], "call differs from preview")
        record={"name":name,"node":node.name,"opening_hex":obj["info"]["opening_hex"],"result":result,"preview":preview}
        report["calls"].append(record); calls.append(record)
        return result
    def update_successors():
        for record in calls:
            result=record["result"];txid=result["transaction"]["txid"]
            live.require(rpc(a,"getTx",[txid]) is not None,"submitted call not confirmed")
            if result["successor"] is not None:
                obj=objects[record["name"]]
                obj["info"]=result["successor"]
                obj["slot"]=output_for(a,txid,obj["info"]["address"])
                live.require(obj["slot"] is not None,"successor missing")
        calls.clear()
    def memory():
        values = receiver_resources(unit)
        report.setdefault("receiver_resource_samples", []).append({"height": a.height(),
            "peak_bytes": int(values["memory.peak"]), "events": values["memory.events"],
            "cgroup": values})
    try:
        b.start("01-receiver")
        report["receiver_enforcement"] = receiver_resources(unit)
        a.start("02-producer",mode="extminer",genesis=True,seeds=[b.seed])
        payer=rpc(a,"walletActiveAddress")["address"];payee=rpc(b,"walletActiveAddress")["address"]
        report.update(payer=payer,payee=payee)
        initial=rpc(a,"getContractProtocol")
        live.require(not initial["active_at_next_block"] and initial["activation_height"]==10,"isolated fork schedule differs")
        mine(8)
        vault=rpc(a,"createObject",[{"kind":"timelocked_vault","owner":payer,"unlock_height":18,"max_fee_micronoid":1000000}])
        report["negative_cases"]["pre_fork_funding"]=rejected(a,"walletFundObject",[vault["opening_hex"],10000000,0])
        mine()
        live.require(rpc(a,"getMiningInfo")["block_reward_micronoid"]==16000000,"next-block reward not switched at boundary")
        live.require(rpc(a,"getMiningInfo")["matrix_cache_classes"]==[],"GUI still requests legacy caches for v2")
        mine()
        checkpoint("fund all six templates and integer programs")
        definitions={
            "payment":{"kind":"refundable_payment","payer":payer,"payee":payee,"expiry_height":18,"max_fee_micronoid":1000000},
            "refund":{"kind":"refundable_payment","payer":payer,"payee":payee,"expiry_height":19,"max_fee_micronoid":1000000},
            "vault":{"kind":"timelocked_vault","owner":payer,"unlock_height":18,"max_fee_micronoid":1000000},
            "allowance":{"kind":"allowance_wallet","spending_key":payee,"recovery_key":payer,"payout_recipient":payee,"recover_at":18,"max_fee_micronoid":1000000,"max_payout_micronoid":1000000,"min_retained_micronoid":1000000},
            "budget":{"kind":"period_budget_wallet","spending_key":payee,"recovery_key":payer,"payout_recipient":payee,"start_height":12,"period_blocks":3,"budget_micronoid":1500000,"recover_at":18,"max_fee_micronoid":1000000,"max_payout_micronoid":1000000,"min_retained_micronoid":1000000},
            "recurring":{"kind":"recurring_payment","payer":payer,"payee":payee,"first_due_height":12,"period_blocks":3,"payment_micronoid":1000000,"recover_at":18,"max_fee_micronoid":1000000},
            "vesting":{"kind":"tranche_vesting","beneficiary":payee,"first_unlock_height":12,"period_blocks":2,"tranche_micronoid":1000000,"mature_at":18,"max_fee_micronoid":1000000},
        }
        for name,state0 in [("counter","0"),("overflow",str(2**64-1))]:
            definitions[name]={"kind":"custom_program","definition":{"state":[state0,"0"],
                "program":[{"opcode":"add","destination":"state0","left":"state0","right":"one","predicate":{"source":"terminal","inverted":True},"immediate":"0"}],
                "claim_authority":payer,"recovery_authority":payer,"claim_recipient":payer,"recovery_recipient":payer,
                "deadline_height":18,"max_fee_micronoid":1000000,"max_payout_micronoid":1000000,"min_retained_micronoid":0,
                "claim_can_continue":True,"claim_can_close":True,"recovery_can_continue":False,"recovery_can_close":True,"unrestricted_payout_recipient":False}}
        for name,definition in definitions.items():
            path=artifacts/(name+'.json')
            source=artifacts/(name+'.definition.json');source.write_text(json.dumps(definition))
            info=cli(a,["create",source,"--out",path]);cli(b,["watch",path])
            funding=cli(a,["fund",path,"10"])
            objects[name]={"info":info,"funding":funding,"path":str(path)}
        mine() # H11
        for obj in objects.values():
            obj["slot"]=output_for(a,obj["funding"]["txid"],obj["info"]["address"])
            live.require(obj["slot"] is not None,"funding missing")
        neg=report["negative_cases"]
        neg["wrong_authority"]=rejected(a,"walletCallObject",[request("payment",terminal=True)])
        neg["early_vault"]=rejected(a,"previewObjectCall",[request("vault",terminal=True)])
        neg["overflow"]=rejected(a,"previewObjectCall",[request("overflow")])
        neg["bad_incarnation"]=rejected(b,"previewObjectCall",[request("payment",terminal=True,creation_id=objects['payment']['slot']['creation_id']+1)])
        neg["oversized_payment"]=rejected(b,"previewObjectCall",[request("allowance",payout={"address":payee,"amount_micronoid":1000001})])
        call(b,"payment",close=True)
        for name in ("allowance","budget","recurring","vesting"): call(b,name,pay=1000000)
        call(a,"counter")
        mine();update_successors() # H12
        live.require(objects["counter"]["info"]["state"][0]=="1","integer counter did not persist")
        live.require(objects["recurring"]["info"]["state"]==["1","15"],"recurring state differs")
        live.require(objects["vesting"]["info"]["state"]==["1000000","14"],"vesting state differs")
        for name in ("budget","recurring","vesting"):
            neg[name+"_early_or_exhausted"]=rejected(b,"previewObjectCall",[request(name,payout={"address":payee,"amount_micronoid":1000000})])
        stale=request("counter",expected_authority=payer)
        reviewed=rpc(a,"previewObjectCall",[stale]);stale.update(expected_call_height=reviewed["call_height"],expected_txid=reviewed["txid"])
        mine() # H13
        neg["stale_review"]=rejected(a,"walletCallObject",[stale])
        call(b,"vesting",pay=1000000);call(a,"counter")
        mine();update_successors() # H14
        call(b,"budget",pay=1000000);call(b,"recurring",pay=1000000)
        mine();update_successors() # H15
        mine(2) # H17
        for name in ("vault","allowance","budget","recurring","counter","overflow"):call(a,name,close=True)
        call(b,"vesting",close=True)
        mine();update_successors() # H18
        memory();b.stop()
        call(a,"refund",close=True)
        mine();update_successors() # H19
        mine(2)
        b.start("03-receiver-suffix",seeds=[a.seed]);converge()
        checkpoint("verify retained call receipts through both nodes")
        report["receipt_checks"]=[]
        for record in report["calls"]:
            txid=record["result"]["transaction"]["txid"]
            for node in (a,b):
                encoded=rpc(node,"exportObjectReceipt",[record["opening_hex"],txid])
                checked=rpc(node,"verifyObjectReceipt",[encoded])
                live.require(checked["valid"] and checked["txid"]==txid,"receipt did not verify")
                (artifacts/(node.name+'-'+txid+'.receipt')).write_bytes(bytes.fromhex(encoded))
                report["receipt_checks"].append({"node":node.name,"txid":txid,"height":checked["height"],"bytes":len(encoded)//2,"descendant":bytes.fromhex(encoded).startswith(b'O1OBJRC5')})
        # Optional pruning pass adds 43 genuine blocks. A quick development run
        # can defer it, but cannot claim complete retention qualification.
        if os.environ.get('NOID_V2_SKIP_PRUNING') != '1':
            mine(43)
            for record in report["calls"]:
                txid=record["result"]["transaction"]["txid"]
                for node in (a,b):
                    encoded=rpc(node,"exportObjectReceipt",[record["opening_hex"],txid])
                    checked=rpc(node,"verifyObjectReceipt",[encoded])
                    live.require(rpc(node,"getBlockDetails",[checked['height']])["retained"] is None,"call body was not pruned")
                    live.require(checked['valid'],"receipt failed after pruning")
            report['pruning']='passed'
        else: report['pruning']='not_run'
        memory();b.stop();b.start("04-receiver-restart",seeds=[a.seed]);converge()
        report.update(status="passed",final_height=a.height())
        checkpoint("complete")
    except Exception as error:
        report.update(status="failed",error=str(error));checkpoint("failed");raise
    finally:
        for node in (a,b): node.request_stop()
        for node in (a,b):
            try:node.finish_stop()
            except Exception as error:report.setdefault("shutdown_errors",[]).append(str(error))
        (BASE/'report.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps({k:v for k,v in report.items() if k not in ('calls','negative_cases','receipt_checks','binary_sha256')}),flush=True)


if __name__ == '__main__': main()

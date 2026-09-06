#!/usr/bin/env python3
"""Fresh high-fan-in P2P handshake test with one hub and many peers.

Every process receives an empty data directory. Spokes are spawned before any
of them is awaited, so their Identify, header probe, and bounded outbound
mempool pull hit the hub concurrently. The inbound hub must not reciprocate
bulk bootstrap work towards every spoke. This scenario does not mine or reuse
chain state; mining/reorg coverage remains in its own live scenarios.
"""

import datetime
import json
import os
import re
import time
from pathlib import Path

import live_two_miner_fork_reorg_scenario as live


ROOT = Path(__file__).resolve().parents[1]
RUN_PARENT = ROOT / "target" / "live-tests"
STAMP = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
BASE = Path(
    os.environ.get(
        "NOID_LIVE_P2P_FAN_IN_DIR",
        str(RUN_PARENT / f"p2p-fan-in-clean-{STAMP}"),
    )
)
BASE_PORT = int(os.environ.get("NOID_LIVE_P2P_FAN_IN_BASE_PORT", "21000"))
PEER_COUNT = int(os.environ.get("NOID_LIVE_P2P_FAN_IN_PEERS", "96"))
IDLE_WATCH_SECONDS = int(os.environ.get("NOID_LIVE_P2P_FAN_IN_IDLE_SECONDS", "135"))

live.BASE = BASE
live.BASE_PORT = BASE_PORT
Node = live.Node
rpc = live.rpc
require = live.require


def log_text(label):
    return (BASE / "logs" / f"{label}.log").read_text(errors="replace")


def logged_peer_id(text):
    match = re.search(r"loaded persistent P2P identity peer=([^\s]+)", text)
    require(match is not None, "startup log has no persistent PeerId")
    assert match is not None
    return match.group(1)


def rss_kib(node):
    if node.proc is None or node.proc.poll() is not None:
        return 0
    try:
        status = Path(f"/proc/{node.proc.pid}/status").read_text()
    except OSError:
        return 0
    match = re.search(r"^VmRSS:\s+(\d+)\s+kB$", status, re.MULTILINE)
    return int(match.group(1)) if match else 0


def pull_requests(text):
    return sum(text.count(marker) for marker in (
        "requesting mempool sync",
        "requesting missing mempool transactions",
        "retrying mempool sync",
    ))


def exchanges_complete(hub_label, spoke_labels):
    hub = log_text(hub_label)
    hub_id = logged_peer_id(hub)
    spoke_ids = {}
    for label in spoke_labels:
        text = log_text(label)
        peer_id = logged_peer_id(text)
        spoke_ids[label] = peer_id
        if not pull_requests(text) or "mempool sync response complete" not in text:
            return False
        if not any(
            hub_id in line and "mempool sync response complete" in line
            for line in text.splitlines()
        ):
            return False
    hub_lines = hub.splitlines()
    for peer_id in spoke_ids.values():
        if not any(
            peer_id in line and "serving mempool sync request" in line
            for line in hub_lines
        ):
            return False
    return {"hub": hub_id, **spoke_ids}


def assert_clean(label, text):
    forbidden = (
        " ERROR ",
        "panicked",
        "P2P network error",
        "max sub-streams reached",
        "Handshake failed: input error",
        "mempool sync request failed — retry limit reached",
    )
    failures = [line for line in text.splitlines() if any(item in line for item in forbidden)]
    require(not failures, f"{label} contains P2P failures: {failures[-10:]}")


def stop_all(nodes):
    errors = []
    for node in reversed(nodes):
        try:
            node.request_stop()
        except Exception as error:  # cleanup must continue for every process
            errors.append(f"request {node.name}: {error}")
    for node in reversed(nodes):
        try:
            node.finish_stop()
        except Exception as error:  # cleanup must continue for every process
            errors.append(f"finish {node.name}: {error}")
    return errors


def main():
    require(live.NODE_BIN.is_file(), f"release node is missing: {live.NODE_BIN}")
    require(
        1 <= PEER_COUNT <= 96,
        "single-group fan-in peers must be between 1 and the 96-peer diversity cap",
    )
    require(not BASE.exists(), f"run directory already exists: {BASE}")
    require(IDLE_WATCH_SECONDS >= 0, "idle observation duration must be nonnegative")

    hub = Node("hub", BASE_PORT, BASE_PORT + 1)
    spokes = [
        Node(f"spoke-{index:03d}", BASE_PORT + 100 + index * 2, BASE_PORT + 101 + index * 2)
        for index in range(PEER_COUNT)
    ]
    nodes = [hub, *spokes]
    for node in nodes:
        require(live.port_is_free(node.p2p_port), f"port occupied: {node.p2p_port}")
        require(live.port_is_free(node.rpc_port), f"port occupied: {node.rpc_port}")

    (BASE / "logs").mkdir(parents=True)
    (RUN_PARENT / "LAST_P2P_FAN_IN_RUN").write_text(str(BASE) + "\n")
    binary_hash = live.sha256(live.NODE_BIN)
    print(f"[run] {BASE}", flush=True)
    print(
        f"[binary] sha256={binary_hash} size={live.NODE_BIN.stat().st_size} peers={PEER_COUNT}",
        flush=True,
    )
    summary = {
        "run_dir": str(BASE),
        "binary_sha256": binary_hash,
        "binary_size": live.NODE_BIN.stat().st_size,
        "peer_count": PEER_COUNT,
        "status": "running",
    }
    error = None
    cleanup_errors = []
    hub_label = "01-hub-fresh-genesis"
    spoke_labels = [f"02-spoke-{index:03d}-fresh" for index in range(PEER_COUNT)]
    try:
        hub.start(hub_label, genesis=True)

        # Spawn the full fan-in first; only then wait for individual RPCs.
        starts = []
        launched = time.monotonic()
        for node, label in zip(spokes, spoke_labels):
            starts.append(node.spawn(label, seeds=[hub.seed]))
        spawn_elapsed = time.monotonic() - launched
        print(f"[spawned] {PEER_COUNT} peers in {spawn_elapsed:.3f}s", flush=True)

        startup_seconds = []
        for node, label, started in zip(spokes, spoke_labels, starts):
            _, elapsed = node.wait_ready(label, started, timeout=300)
            startup_seconds.append(round(elapsed, 3))

        live.wait_value(
            "hub accepts full simultaneous fan-in",
            lambda: int(rpc(hub.rpc_port, "getPeerCount")) >= PEER_COUNT,
            timeout=180,
            interval=0.5,
        )
        live.wait_value(
            "every spoke remains connected to at least one peer",
            lambda: all(int(rpc(node.rpc_port, "getPeerCount")) >= 1 for node in spokes),
            timeout=180,
            interval=0.5,
        )
        peer_ids = live.wait_value(
            "all client-to-hub mempool exchanges finish under fan-in",
            lambda: exchanges_complete(hub_label, spoke_labels),
            timeout=240,
            interval=0.5,
        )
        require(len(set(peer_ids.values())) == PEER_COUNT + 1, "PeerId collision in fan-in")

        # A completed empty scan is not a subscription to a retained seed.
        # Observe longer than the former 120-second idle-refresh regression.
        completed_requests = {label: pull_requests(log_text(label)) for label in spoke_labels}
        require(pull_requests(log_text(hub_label)) == 0, "hub reciprocated inbound mempool bootstrap")
        idle_started = time.monotonic()
        max_rpc_ms = 0.0
        while time.monotonic() - idle_started < IDLE_WATCH_SECONDS:
            before = time.monotonic()
            rpc(hub.rpc_port, "getPeerCount", timeout=5)
            max_rpc_ms = max(max_rpc_ms, (time.monotonic() - before) * 1000)
            for label, count in completed_requests.items():
                require(pull_requests(log_text(label)) == count,
                        f"{label} resumed polling after its empty response")
            require(pull_requests(log_text(hub_label)) == 0, "hub started reverse mempool polling")
            time.sleep(1)
        summary["idle_observation_s"] = IDLE_WATCH_SECONDS
        summary["idle_extra_pull_requests"] = 0
        summary["hub_reverse_pull_requests"] = 0
        summary["max_sampled_hub_rpc_ms"] = round(max_rpc_ms, 3)

        # Let late swarm events settle before judging retry exhaustion or
        # connection churn, then capture process memory while all remain live.
        time.sleep(3)
        hub_peers = int(rpc(hub.rpc_port, "getPeerCount"))
        require(hub_peers >= PEER_COUNT, f"hub lost peers after exchange: {hub_peers}")
        rss = {node.name: rss_kib(node) for node in nodes}
        summary.update(
            {
                "spawn_s": round(spawn_elapsed, 3),
                "startup_s_max": max(startup_seconds),
                "hub_peer_count": hub_peers,
                "unique_peer_ids": len(set(peer_ids.values())),
                "aggregate_rss_kib": sum(rss.values()),
                "max_process_rss_kib": max(rss.values()),
            }
        )
    except Exception as caught:
        error = caught
        summary["status"] = "failed"
        summary["error"] = str(caught)
        print(f"[FAIL] {caught}", flush=True)
    finally:
        cleanup_errors = stop_all(nodes)

    try:
        all_logs = {hub_label: log_text(hub_label)}
        all_logs.update({label: log_text(label) for label in spoke_labels if (BASE / "logs" / f"{label}.log").exists()})
        for label, text in all_logs.items():
            assert_clean(label, text)
        retry_lines = sum(
            text.count("mempool sync request failed — retry scheduled")
            + text.count("mempool pull failed — bounded recovery updated")
            for text in all_logs.values()
        )
        summary["scheduled_retries"] = retry_lines
        summary["clean_logs"] = len(all_logs)
        require(not cleanup_errors, f"cleanup failures: {cleanup_errors}")
        if error is None:
            summary["status"] = "passed"
            print(
                f"[PASS] fresh {PEER_COUNT}-peer fan-in; retries={retry_lines} hub_peers={summary['hub_peer_count']}",
                flush=True,
            )
    except Exception as caught:
        if error is None:
            error = caught
            print(f"[FAIL] {caught}", flush=True)
        summary["status"] = "failed"
        summary["error"] = str(error)

    (BASE / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(f"[summary] {BASE / 'summary.json'}", flush=True)
    if error is not None:
        raise error


if __name__ == "__main__":
    main()

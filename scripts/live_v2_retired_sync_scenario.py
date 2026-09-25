#!/usr/bin/env python3
"""Cold P2P synchronization with and without embedded legacy matrices.

Requires separately built transition/retired isolated binaries and a freshly
verified retirement certificate for the source fixture's exact fork origin.
Only source bytes are copied; both receiving nodes begin with empty data.
"""
import json
import os
from pathlib import Path
import subprocess
import time

import live_v2_contract_scenario as contracts

live = contracts.live
BASE = contracts.BASE
ROOT = contracts.ROOT
rpc = contracts.rpc


class Node(contracts.Node):
    def __init__(self, *args, binary, constrained=False, **kwargs):
        self.binary = binary
        self.constrained = constrained
        super().__init__(*args, **kwargs)

    def spawn(self, *args, **kwargs):
        binary = contracts.NODE
        backend = os.environ.get('NOID_CPU_BACKEND')
        try:
            contracts.NODE = self.binary
            if self.constrained:
                os.environ['NOID_CPU_BACKEND'] = 'pclmul'
            return super().spawn(*args, **kwargs)
        finally:
            contracts.NODE = binary
            if backend is None:
                os.environ.pop('NOID_CPU_BACKEND', None)
            else:
                os.environ['NOID_CPU_BACKEND'] = backend


def main():
    devices = [line.split(':')[0].strip() for line in Path('/proc/net/dev').read_text().splitlines()[2:]]
    live.require(devices == ['lo'], 'use an isolated loopback-only network namespace')
    subprocess.run(['ip', 'link', 'set', 'lo', 'up'], check=True)
    source = Path(os.environ['NOID_V2_SYNC_SOURCE']).resolve()
    transition = Path(os.environ['NOID_V2_TRANSITION_NODE']).resolve()
    retired_binary = Path(os.environ['NOID_V2_RETIRED_NODE']).resolve()
    certificate = Path(os.environ['NOID_V2_RETIREMENT_CERTIFICATE']).resolve()
    prior = json.loads((source / 'report.json').read_text())
    gui_call = json.loads((source / 'gui-call-verification.json').read_text())
    live.require(prior['status'] == 'passed' and not prior.get('shutdown_errors'), 'source fixture did not pass and stop')
    live.require(not BASE.exists(), f'fresh directory required: {BASE}')
    BASE.mkdir(parents=True)
    (BASE / 'logs').mkdir()
    subprocess.run(['cp', '-a', '--reflink=auto', '--sparse=always', str(source / 'producer'), str(BASE / 'source')], check=True)
    origin_files = list((BASE / 'source/data/fork-origins').glob('*.origin'))
    live.require(len(origin_files) == 1, 'source must have one selected fork origin')
    original = origin_files[0].read_bytes()
    replacement = certificate.read_bytes()
    live.require(original.startswith(b'O1V2OR01') and replacement.startswith(b'O1V2OR02'), 'unexpected origin formats')
    # This changes only transport evidence in the copied serving node. Every
    # receiver still checks the full certificate against its own pinned keys.
    origin_files[0].write_bytes(replacement)
    origin_name = origin_files[0].name
    live.BASE = BASE
    a = Node('source', 26300, 26301, binary=transition)
    def constrained(name, p2p, port, binary):
        unit = f'noid-v2-sync-{name}-{os.getpid()}'
        return Node(name, p2p, port, binary=binary, constrained=True,
            command_prefix=('systemd-run', '--user', '--scope', '--quiet', f'--unit={unit}',
                            '-p', 'MemoryMax=8G', '-p', 'MemorySwapMax=0', 'taskset', '-c', '0,2,4,6')), unit
    b, normal_unit = constrained('receiver', 26310, 26311, transition)
    c, retired_unit = constrained('retired', 26320, 26321, retired_binary)
    report = {'status': 'running', 'source_tip': prior['final_tip'], 'checks': [],
        'source_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'script_sha256': {Path(p).name: live.sha256(p) for p in (__file__, contracts.__file__, live.__file__)},
        'binary_sha256': {p.name: live.sha256(p) for p in (transition, retired_binary)},
        'certificate_sha256': live.sha256(certificate), 'certificate_bytes': len(replacement),
        'receiver_cpu_affinity': [0, 2, 4, 6], 'receiver_backend': 'pclmul',
        'receiver_memory_max': 8 * 1024**3, 'receiver_swap_max': 0}
    portable_receipt = None

    def checkpoint(stage):
        report['stage'] = stage
        (BASE / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        print(f'[stage] {stage}', flush=True)

    def resources(unit):
        group = subprocess.check_output(['systemctl', '--user', 'show', unit + '.scope', '-p', 'ControlGroup', '--value'], text=True).strip()
        path = Path('/sys/fs/cgroup') / group.lstrip('/')
        return {name: (path / name).read_text().strip() for name in ('memory.current', 'memory.peak', 'memory.events', 'cpu.stat')}

    def check(node, unit, label):
        nonlocal portable_receipt
        live.wait_value(f'{label}: exact source tip', lambda: node.info() == prior['final_tip'], 1200)
        protocol = rpc(node, 'getContractProtocol')
        live.require(protocol['active_at_next_block'] and protocol['runtime_available']
                     and protocol['next_block_time_seconds'] == 30, 'cold node did not activate contract rules')
        wanted = json.loads((source / 'snapshot.json').read_text())['states']
        current = next(item['object'] for item in wanted['states'] if item['has_balance'])
        instances = rpc(node, 'getObjectInstances', [current['opening_hex'], 0, 32])
        live.require(instances == gui_call['successor_instances'], 'cold node contract State differs')
        # The original opening is retained in the source's saved GUI library.
        library = json.loads((source / 'producer/data/wallet.contracts.json').read_text())
        old = next(item['info'] for item in library if item['info']['state'] == ['0', '0'])
        receipt = rpc(node, 'exportObjectReceipt', [old['opening_hex'], gui_call['txid']])
        checked = rpc(node, 'verifyObjectReceipt', [receipt])
        live.require(checked['valid'] and checked['txid'] == gui_call['txid'], 'cold node receipt did not verify')
        portable_receipt = receipt
        path = node.data_dir / 'fork-origins' / origin_name
        live.require(path.read_bytes() == replacement, 'received certificate was not retained exactly')
        if node is c:
            rows = list((node.data_dir / 'history-step-cache').rglob('*.packed-r1cs.zst'))
            live.require(not rows, 'retired node materialized old matrix rows')
        observed = {'label': label, 'height': node.height(), 'tip': node.info(),
                    'receipt_verified': True, 'resource': resources(unit)}
        report['checks'].append(observed)
        return observed

    try:
        a.start('01-serving-source')
        live.require(a.info() == prior['final_tip'], 'serving fixture tip changed')
        checkpoint('cold transition binary: genesis to pruned v2 history')
        live.require(not b.data_dir.exists(), 'transition receiver is not empty')
        started = time.monotonic()
        b.start('02-cold-transition', seeds=[a.seed])
        observed = check(b, normal_unit, 'transition cold sync')
        observed['startup_and_sync_seconds'] = time.monotonic() - started
        live.require('snapshot install completed' in b.log_path.read_text(errors='replace'), 'transition did not exercise snapshot bootstrap')
        b.stop()

        checkpoint('cold retired binary: obtain proof without legacy matrices')
        live.require(not c.data_dir.exists(), 'retired receiver is not empty')
        started = time.monotonic()
        c.start('03-cold-retired', seeds=[a.seed])
        observed = check(c, retired_unit, 'retired cold sync')
        observed['startup_and_sync_seconds'] = time.monotonic() - started
        live.require('snapshot install completed' in c.log_path.read_text(errors='replace'), 'retired did not exercise snapshot bootstrap')
        c.stop()
        checkpoint('retired restart verifies from its retained certificate')
        a.stop()
        c.start('04-retired-restart')
        check(c, retired_unit, 'retired offline restart')
        live.require('peer connected' not in c.log_path.read_text(errors='replace'), 'offline check contacted a peer')
        c.stop()

        for label, damaged in [('missing', None), ('legacy-format', original), ('corrupted', replacement[:len(replacement)//2] + bytes([replacement[len(replacement)//2] ^ 1]) + replacement[len(replacement)//2+1:])]:
            checkpoint(f'reject {label} local evidence while no provider is running')
            path = c.data_dir / 'fork-origins' / origin_name
            if damaged is None:
                path.unlink()
            else:
                path.write_bytes(damaged)
            c.start(f'05-retired-reject-{label}')
            rejection = contracts.rejected(c, 'verifyObjectReceipt', [portable_receipt])
            live.require(c.info() == prior['final_tip'], 'failed verification changed the selected tip')
            live.require('peer connected' not in c.log_path.read_text(errors='replace'), 'negative check contacted a peer')
            report.setdefault('offline_rejections', []).append({'label': label, 'error': rejection})
            c.stop()

            checkpoint(f'recover {label} local evidence at the unchanged selected tip')
            a.start(f'06-serving-{label}')
            c.start(f'06-retired-recover-{label}', seeds=[a.seed])
            live.wait_value(f'{label}: authenticated certificate replacement',
                lambda: path.exists() and path.read_bytes() == replacement, 300)
            check(c, retired_unit, f'same-tip {label} recovery')
            live.require('canonical fork origin recovered without advancing the tip' in c.log_path.read_text(errors='replace'),
                         'no same-tip recovery completion evidence')
            c.stop()
            live.require(a.info() == prior['final_tip'], 'provider tip changed during recovery')
            a.stop()
        report.update(status='passed', final_tip=prior['final_tip'], offline_provider_stopped=True)
        checkpoint('complete')
    except Exception as error:
        report.update(status='failed', error=str(error))
        checkpoint('failed')
        raise
    finally:
        for node in (a, b, c):
            node.request_stop()
        for node in (a, b, c):
            try:
                node.finish_stop()
            except Exception as error:
                report.setdefault('shutdown_errors', []).append(str(error))
        (BASE / 'report.json').write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()

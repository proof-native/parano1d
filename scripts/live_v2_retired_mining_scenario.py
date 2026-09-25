#!/usr/bin/env python3
"""Qualify a retired node as verifier and producer after both legacy classes.

Only copies a stopped, verified joint-produce fixture. All extensions use real
daemons, wallet authorization, external nonce work and P2P. The retired node
has four logical CPUs, an 8 GiB memory cap and no swap.
"""
import json
import os
from pathlib import Path
import secrets
import subprocess
import time

import live_v2_contract_scenario as contracts
from live_v2_retired_sync_scenario import Node

live, BASE, ROOT, rpc = contracts.live, contracts.BASE, contracts.ROOT, contracts.rpc


def main():
    devices = [line.split(':')[0].strip() for line in Path('/proc/net/dev').read_text().splitlines()[2:]]
    live.require(devices == ['lo'], 'use an isolated loopback-only network namespace')
    subprocess.run(['ip', 'link', 'set', 'lo', 'up'], check=True)
    source = Path(os.environ['NOID_V2_FULL_ORIGIN_SOURCE']).resolve()
    certificate_dir = Path(os.environ['NOID_V2_FULL_RETIREMENT']).resolve()
    transition = Path(os.environ['NOID_V2_TRANSITION_NODE']).resolve()
    retired_binary = Path(os.environ['NOID_V2_RETIRED_NODE']).resolve()
    prior = json.loads((source / 'production.json').read_text())
    certificate_report = json.loads((certificate_dir / 'retirement-release.json').read_text())
    live.require(prior['complete'] and prior['durable_reopen'] and prior['tip'] == 17, 'invalid source fixture')
    live.require(certificate_report['status'] == 'passed'
                 and certificate_report['v2_bank'] == prior['bank']
                 and [item['class'] for item in certificate_report['classes']] == [0, 1]
                 and all(item['proof_bytes'] > 0 for item in certificate_report['classes']), 'both legacy obligations in the same bank required')
    resume_path = os.environ.get('NOID_V2_RETIRED_RESUME_SOURCE')
    resume_source = Path(resume_path).resolve() if resume_path else None
    resume = json.loads((resume_source / 'report.json').read_text()) if resume_source else None
    if resume:
        # A completed real chain can be reused after a harness-only retention
        # window error. Other failures are not eligible for this continuation.
        live.require(resume['status'] == 'failed'
                     and resume['error'] == 'timeout waiting for old call body pruned; last=False'
                     and not resume.get('shutdown_errors')
                     and resume['certificate_origin'] == certificate_report['origin_binding'],
                     'resume requires the stopped retention-window checkpoint')
    live.require(not BASE.exists(), f'fresh directory required: {BASE}')
    BASE.mkdir(parents=True)
    (BASE / 'logs').mkdir()
    key = BASE / 'mining.key'
    with key.open('x') as file:
        file.write(secrets.token_hex(32) + '\n')
    key.chmod(0o600)
    certificate = (certificate_dir / 'retired-v2-origin.bin').read_bytes()
    live.require(certificate.startswith(b'O1V2OR02'), 'retirement certificate required')
    name = certificate_report['origin_binding'] + '.origin'
    live.require((source / 'producer-origins' / name).is_file(), 'certificate belongs to a different source origin')
    for source_name, node_name in [('producer', 'producer'), ('receiver', 'peer')]:
        destination = BASE / node_name / 'data'
        if resume_source:
            subprocess.run(['cp', '-a', '--reflink=auto', '--sparse=always',
                            str(resume_source / node_name), str(destination.parent)], check=True)
        else:
            destination.parent.mkdir()
            subprocess.run(['cp', '-a', '--reflink=auto', '--sparse=always', str(source / source_name), str(destination)], check=True)
        origins = destination / 'fork-origins'
        origins.mkdir(exist_ok=True)
        if resume_source:
            live.require((origins / name).read_bytes() == certificate, 'resumed certificate differs')
        (origins / name).write_bytes(certificate)
    live.BASE = BASE
    a = Node('producer', 26300, 26301, binary=transition,
             command_prefix=('taskset', '-c', '1,3,5,7,8,9,10,11'))
    b = Node('peer', 26310, 26311, binary=transition)
    unit = f'noid-v2-retired-producer-{os.getpid()}'
    c = Node('retired', 26320, 26321, binary=retired_binary, constrained=True,
        command_prefix=('systemd-run', '--user', '--scope', '--quiet', f'--unit={unit}',
            '-p', 'MemoryMax=8G', '-p', 'MemorySwapMax=0', 'taskset', '-c', '0,2,4,6'))
    source_height = resume['blocks'][-1]['last'] if resume else 17
    report = {'status': 'running', 'source_height': source_height, 'blocks': [], 'checks': [],
        'source_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'script_sha256': {Path(p).name: live.sha256(p) for p in
            (__file__, Path(__file__).with_name('live_v2_retired_sync_scenario.py'), contracts.__file__, live.__file__)},
        'binary_sha256': {p.name: live.sha256(p) for p in (transition, retired_binary, contracts.MINER)},
        'certificate_bytes': len(certificate), 'certificate_origin': certificate_report['origin_binding'],
        'certificate_sha256': live.sha256(certificate_dir / 'retired-v2-origin.bin'),
        'retired_node_cpu_affinity': [0, 2, 4, 6], 'retired_backend': 'pclmul',
        'retired_memory_max': 8 * 1024**3, 'retired_swap_max': 0, 'external_nonce_worker_threads': 2}
    if resume:
        report['resumed_checkpoint'] = {
            'report_sha256': live.sha256(resume_source / 'report.json'),
            'status': resume['status'], 'error': resume['error'],
            'source_head': resume['source_head'], 'script_sha256': resume['script_sha256'],
            'binary_sha256': resume['binary_sha256'], 'blocks': resume['blocks']}

    def checkpoint(stage):
        report['stage'] = stage
        (BASE / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        print(f'[stage] {stage}', flush=True)

    def resources(label):
        group = subprocess.check_output(['systemctl', '--user', 'show', unit + '.scope', '-p', 'ControlGroup', '--value'], text=True).strip()
        path = Path('/sys/fs/cgroup') / group.lstrip('/')
        sample = {'label': label, 'height': c.height(), **{
            name: (path / name).read_text().strip() for name in ('memory.current', 'memory.peak', 'memory.events', 'cpu.stat')}}
        events = dict(line.split() for line in sample['memory.events'].splitlines())
        live.require(all(events.get(event) == '0' for event in ('max', 'oom', 'oom_kill')), 'retired node exceeded its envelope')
        report.setdefault('retired_resources', []).append(sample)

    def converge(left, right):
        live.wait_value('exact accepted tip', lambda: live.exact_tip(left, right), 1200)

    def mine(node, receiver, count=1):
        first = node.height() + 1
        checkpoint(f'{node.name} proves and mines H{first}..{first + count - 1}')
        start = time.monotonic()
        with (BASE / 'logs' / f'external-{node.name}-{first}.log').open('w') as log:
            result = subprocess.run([str(contracts.MINER), '--rpc', f'http://127.0.0.1:{node.rpc_port}',
                '--key-file', str(key), '--threads', '2', '--rpc-timeout', '600', '--blocks', str(count)],
                cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=max(1200, count * 180))
        live.require(result.returncode == 0 and node.height() == first + count - 1, 'finite mining failed')
        converge(node, receiver)
        report['blocks'].append({'producer': node.name, 'first': first, 'last': node.height(),
                                 'production_and_delivery_seconds': time.monotonic() - start})

    def call(opening):
        slots = rpc(a, 'getObjectInstances', [opening['opening_hex'], 0, 2])['slots']
        live.require(len(slots) == 1, 'one contract instance required')
        slot = slots[0]
        request = {'opening_hex': opening['opening_hex'], 'slot_index': slot['slot_index'],
                   'creation_id': slot['creation_id'], 'terminal': False, 'fee_micronoid': 0,
                   'expected_authority': rpc(a, 'walletActiveAddress')['address']}
        preview = rpc(a, 'previewObjectCall', [request])
        request.update(expected_txid=preview['txid'], expected_call_height=preview['call_height'],
                       expected_recovery=preview['recovery'])
        result = rpc(a, 'walletCallObject', [request])
        live.require(result['transaction']['txid'] == preview['txid'] and result['successor'] == preview['successor'],
                     'authorized call differs from review')
        return result

    def checked_receipt(node, receipt, txid):
        checked = rpc(node, 'verifyObjectReceipt', [receipt])
        live.require(checked['valid'] and checked['txid'] == txid, 'portable receipt rejected')
        report['checks'].append({'node': node.name, 'height': node.height(), 'txid': txid, 'receipt_verified': True})

    try:
        b.start('01-existing-peer')
        a.start('02-transition-producer', mode='extminer', genesis=True, seeds=[b.seed])
        converge(a, b)
        live.require(a.height() == source_height, 'copied origin fixture height differs')
        live.require(rpc(a, 'getContractProtocol')['activation_height'] == 10, 'isolated schedule required')
        if resume:
            opening, first = resume['opening'], resume['first_call']
            first_call_height = resume['first_call_height']
        else:
            mine(a, b)  # A new wallet obtains its first ordinary mining output at H18.
            owner = rpc(a, 'walletActiveAddress')['address']
            definition = {'kind': 'custom_program', 'definition': {
                'state': ['0', '0'],
                'program': [{'opcode': 'add', 'destination': 'state0', 'left': 'state0', 'right': 'one',
                             'predicate': {'source': 'terminal', 'inverted': True}, 'immediate': '0'}],
                'claim_authority': owner, 'recovery_authority': owner,
                'claim_recipient': owner, 'recovery_recipient': owner,
                'deadline_height': 1000000, 'max_fee_micronoid': 1000000,
                'max_payout_micronoid': 1000000, 'min_retained_micronoid': 0,
                'claim_can_continue': True, 'claim_can_close': True,
                'recovery_can_continue': False, 'recovery_can_close': True,
                'unrestricted_payout_recipient': False}}
            opening = rpc(a, 'createObject', [definition])
            rpc(b, 'walletWatchObject', [opening['opening_hex']])
            funded = rpc(a, 'walletFundObject', [opening['opening_hex'], 10000000, 0])
            live.wait_value('contract funding admitted', lambda: rpc(a, 'getMempoolEntry', [funded['txid']]) is not None, 120)
            mine(a, b)
            first = call(opening)
            mine(a, b)
            first_call_height = a.height()
        first_txid = first['transaction']['txid']
        first_receipt = rpc(a, 'exportObjectReceipt', [opening['opening_hex'], first_txid])
        checked_receipt(a, first_receipt, first_txid)
        live.require(first['successor']['state'] == ['1', '0'], 'first counter update failed')
        report.update(opening=opening, first_call=first, first_call_height=first_call_height)
        # Full block serving retains 42 blocks: 18 finalized + 18 recent + 6.
        # The 18-block snapshot suffix is a distinct, shorter window.
        target = first_call_height + 43
        if a.height() < target:
            mine(a, b, target - a.height())
        live.wait_value('old call body pruned', lambda: rpc(a, 'getBlockDetails', [first_call_height])['retained'] is None, 180)
        first_receipt = rpc(a, 'exportObjectReceipt', [opening['opening_hex'], first_txid])
        b.stop()
        checkpoint('empty retired node authenticates both legacy obligations over P2P')
        live.require(not c.data_dir.exists(), 'retired receiver must start empty')
        start = time.monotonic()
        c.start('03-retired-cold-sync', seeds=[a.seed])
        converge(a, c)
        report['retired_cold_sync_seconds'] = time.monotonic() - start
        live.require('snapshot install completed' in c.log_path.read_text(errors='replace'), 'snapshot path not exercised')
        live.require((c.data_dir / 'fork-origins' / name).read_bytes() == certificate, 'full certificate was not retained')
        checked_receipt(c, first_receipt, first_txid)
        rpc(c, 'walletWatchObject', [first['successor']['opening_hex']])
        live.require(rpc(c, 'getObjectInstances', [first['successor']['opening_hex'], 0, 2])['slots'], 'cold successor balance missing')
        resources('cold receiver and pruned receipt verification')
        c.stop()

        checkpoint('retired binary becomes a normal external-template producer')
        c.start('04-retired-producer', mode='extminer', seeds=[a.seed])
        converge(a, c)
        mine(c, a)
        produced_heights = [c.height()]
        resources('retired empty block production')
        second = call(first['successor'])
        second_txid = second['transaction']['txid']
        live.wait_value('contract reached retired producer', lambda: rpc(c, 'getMempoolEntry', [second_txid]) is not None, 120)
        mine(c, a)
        produced_heights.append(c.height())
        live.require(second['successor']['state'] == ['2', '0'], 'retired-produced counter update failed')
        second_receipt = rpc(a, 'exportObjectReceipt', [first['successor']['opening_hex'], second_txid])
        checked_receipt(a, second_receipt, second_txid)
        checked_receipt(c, second_receipt, second_txid)
        resources('retired contract block production')
        live.require(not list((c.data_dir / 'history-step-cache').rglob('*.packed-r1cs.zst')), 'legacy matrices appeared')
        final_tip = c.info()
        a.stop()
        c.stop()
        c.start('05-retired-offline-restart')
        live.require(c.info() == final_tip, 'retired producer restart changed its tip')
        checked_receipt(c, first_receipt, first_txid)
        checked_receipt(c, second_receipt, second_txid)
        live.require('peer connected' not in c.log_path.read_text(errors='replace'), 'offline restart contacted a peer')
        resources('offline producer restart')
        report.update(status='passed', final_tip=final_tip, second_call=second,
                      pruned_call_verified=True, retired_produced_heights=produced_heights)
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

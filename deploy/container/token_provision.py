#!/usr/bin/python3
"""Explicit weaker-admission v0. Stock Mesh, no certificate issuance or TLS bypass.

Only used by the stock worker image. A join token is delivered over host-key-pinned
SSH stdin and written once to a private file. Output is a bounded public status;
Mesh stdout/stderr are suppressed because they may contain an invite.
"""
import json
import os
import signal
import struct
import subprocess
import sys
import time
import urllib.request
import vast_provision as common
import token_status

MAGIC = b'M8S-TOKEN-JOIN-V0\n'
FIELDS = {'schema', 'instance_id', 'image_sha256', 'model', 'mesh_id', 'lifetime_seconds'}


class WorkerStopped(Exception):
    """Stop request that cannot be mistaken for a status-probe I/O failure."""



def packet(stream):
    common.require(common.exact(stream, len(MAGIC)) == MAGIC)
    size = struct.unpack('>I', common.exact(stream, 4))[0]
    common.require(0 < size <= 8192)
    config = json.loads(common.exact(stream, size), object_pairs_hook=common.unique)
    common.require(type(config) is dict and set(config) == FIELDS)
    common.require(config['schema'] == 'm8s.token-join/v0')
    common.require(type(config['instance_id']) is int and 0 < config['instance_id'] < 2**64)
    common.require(type(config['lifetime_seconds']) is int and 1 <= config['lifetime_seconds'] <= 1500)
    common.require(config['model'] == common.QWEN38_MODEL_REF)
    for field, sizes in [('image_sha256', (64,)), ('mesh_id', (16, 32, 64))]:
        value = config[field]
        common.require(type(value) is str and len(value) in sizes and all(c in '0123456789abcdef' for c in value))
    size = struct.unpack('>I', common.exact(stream, 4))[0]
    common.require(0 < size <= 16384)
    token = common.exact(stream, size)
    common.require(all(33 <= b <= 126 for b in token) and stream.read(1) == b'')
    return config, token


def command(config):
    return ['/opt/mesh/mesh-llm', 'serve', '--model', config['model'],
            '--join-file', str(common.ROOT / 'join-token'), '--port', '9337',
            '--console', '3131', '--no-draft', '--ctx-size', '4096', '--parallel', '1', '--bind-ip', '0.0.0.0', '--bind-port', '40100']


def start_worker(config):
    return subprocess.Popen(command(config), stdin=subprocess.DEVNULL,
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                 start_new_session=True,
                                 env={'PATH': '/usr/local/nvidia/bin:/usr/bin:/bin',
                                      'LD_LIBRARY_PATH': '/usr/local/nvidia/lib64:/usr/local/nvidia/lib',
                                      'HOME': str(common.ROOT),
                                      'MESH_LLM_NODE_KEY_PATH': str(common.ROOT / 'node.key'),
                                      'MESH_LLM_NATIVE_RUNTIME_BUNDLE_DIR': '/opt/mesh/native-runtimes',
                                      'MESH_LLM_NO_DEFAULT_PLUGINS': '1',
                                      'HF_HUB_DISABLE_IMPLICIT_TOKEN': 'true',
                                      'HF_HUB_CACHE': str(common.ROOT / 'hf-cache'),
                                      'MESH_HF_RETRY_MAX_ATTEMPTS': '0',
                                      'HF_XET_CLIENT_RETRY_MAX_ATTEMPTS': '0',
                                      'MESH_LLM_ANALYTICS': '0', 'DO_NOT_TRACK': '1'})


def stop_worker(child):
    # Reap the whole owned session before reuse or exit, including descendants.
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        child.wait(timeout=3)
    except subprocess.TimeoutExpired:
        pass
    finally:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait()


def recoverable_download(status):
    metrics = token_status.runtime_metrics(status)
    return (metrics.get('runtime_error_download') is True
            and metrics.get('hosted_model_count') == 0
            and metrics.get('requested_model_count') == 1
            and metrics.get('node_loading') is False
            and metrics.get('node_serving') is False
            and not any(metrics.get(key) for key in (
                'runtime_error_auth', 'runtime_error_disk',
                'runtime_error_memory', 'runtime_error_native')))


def receive():
    if os.environ.get('SSH_ORIGINAL_COMMAND') == 'status-v0':
        common.require(os.geteuid() == common.UID)
        print(json.dumps(token_status.read(common.ROOT)), flush=True)
        return
    common.require(os.geteuid() == common.UID and os.environ.get('SSH_ORIGINAL_COMMAND') == 'join-v0')
    signal.alarm(30)
    common.write_new(common.ROOT / 'provision-consumed', b'token-join-v0\n')
    common.sync_directory(common.ROOT)
    config, token = packet(sys.stdin.buffer)
    common.require(str(config['instance_id']) == (common.ROOT / 'instance-id').read_text())
    common.write_new(common.ROOT / 'join-token', token)
    del token
    common.sync_directory(common.ROOT)
    signal.alarm(0)
    started = time.monotonic()
    deadline = started + config['lifetime_seconds']
    observed = started
    last_snapshot = float('-inf')
    child = None
    def stop(_signal, _frame):
        raise WorkerStopped('worker stopped')
    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGHUP, stop)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    announced = False
    announced_node = None
    download_restarts = 0
    try:
        child = start_worker(config)
        token_status.snapshot(common.ROOT, 'starting', 0, child)
        while child.poll() is None:
            observed = time.monotonic()
            if observed >= deadline:
                break
            if observed - last_snapshot >= 1:
                api_reachable, mesh_matches, identity_ready = False, False, False
                status = None
                try:
                    with opener.open('http://127.0.0.1:3131/api/status', timeout=1) as r:
                        raw = r.read(65537)
                    common.require(len(raw) <= 65536)
                    status = json.loads(raw)
                    common.require(type(status) is dict)
                    api_reachable = True
                    mesh_matches = status.get('mesh_id') == config['mesh_id']
                    node = status.get('node_id')
                    common.require(type(node) is str and len(node) in (10, 64) and all(c in '0123456789abcdef' for c in node))
                    common.require(mesh_matches)
                    identity_ready = True
                    if not announced:
                        print(json.dumps({'schema': 'm8s.token-worker/v0', 'node_id': node,
                                          'mesh_id': config['mesh_id'], 'certificate_verified': False}), flush=True)
                        announced = True
                        announced_node = node
                    elif node != announced_node:
                        raise RuntimeError('Worker identity changed during recovery')
                except (ValueError, OSError):
                    pass  # Admission/readiness remains unconfirmed; never weaken policy.
                phase = 'joined' if identity_ready else ('waiting_for_mesh' if api_reachable else 'starting')
                token_status.snapshot(common.ROOT, phase, observed - started, child, api_reachable, mesh_matches, status, download_restarts)
                last_snapshot = observed
                # Retry only a failed download, once, without rebasing the deadline.
                # Stock Mesh resumes the same private partial cache on restart.
                if (identity_ready and download_restarts == 0
                        and deadline - time.monotonic() > 30
                        and recoverable_download(status)):
                    download_restarts += 1
                    stop_worker(child)
                    child = None
                    if time.monotonic() >= deadline:
                        break
                    child = start_worker(config)

            time.sleep(0.25)
    finally:
        try:
            if child is not None:
                stop_worker(child)
        finally:
            (common.ROOT / 'join-token').unlink(missing_ok=True)
            try:
                token_status.snapshot(common.ROOT, 'stopped', observed - started, child, download_restarts=download_restarts)
            except OSError:
                pass  # Observability cannot prevent teardown or token erasure.
    common.require(announced)


if __name__ == '__main__':
    try:
        common.require(len(sys.argv) == 2 and sys.argv[1] in ('serve', 'receive'))
        if sys.argv[1] == 'serve':
            common.serve('token_provision.py')
        else:
            receive()
    except Exception:
        print('Token worker stopped; owner cleanup remains required', file=sys.stderr)
        sys.exit(1)

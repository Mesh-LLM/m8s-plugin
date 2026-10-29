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

MAGIC = b'M8S-TOKEN-JOIN-V0\n'
FIELDS = {'schema', 'instance_id', 'image_sha256', 'model', 'mesh_id', 'lifetime_seconds'}


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


def receive():
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
    deadline = time.monotonic() + config['lifetime_seconds']
    child = None
    def stop(_signal, _frame):
        raise InterruptedError('worker stopped')
    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGHUP, stop)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    announced = False
    try:
        child = subprocess.Popen(command(config), stdin=subprocess.DEVNULL,
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                 start_new_session=True,
                                 env={'PATH': '/usr/bin:/bin', 'HOME': str(common.ROOT),
                                      'MESH_LLM_NATIVE_RUNTIME_BUNDLE_DIR': '/opt/mesh/native-runtimes',
                                      'MESH_LLM_NO_DEFAULT_PLUGINS': '1',
                                      'HF_HUB_DISABLE_IMPLICIT_TOKEN': 'true',
                                      'HF_HUB_CACHE': str(common.ROOT / 'hf-cache'),
                                      'MESH_HF_RETRY_MAX_ATTEMPTS': '0',
                                      'HF_XET_CLIENT_RETRY_MAX_ATTEMPTS': '0',
                                      'MESH_LLM_ANALYTICS': '0', 'DO_NOT_TRACK': '1'})
        while child.poll() is None and time.monotonic() < deadline:
            if not announced:
                try:
                    with opener.open('http://127.0.0.1:3131/api/status', timeout=1) as r:
                        raw = r.read(65537)
                    common.require(len(raw) <= 65536)
                    status = json.loads(raw)
                    node = status.get('node_id')
                    common.require(type(node) is str and len(node) in (10, 64) and all(c in '0123456789abcdef' for c in node))
                    common.require(status.get('mesh_id') == config['mesh_id'])
                    print(json.dumps({'schema': 'm8s.token-worker/v0', 'node_id': node,
                                      'mesh_id': config['mesh_id'], 'certificate_verified': False}), flush=True)
                    announced = True
                except (ValueError, OSError):
                    pass  # Admission/readiness remains unconfirmed; never weaken policy.
            time.sleep(0.25)
    finally:
        try:
            if child is not None:
                # A host exit may leave children in its session. Always stop
                # the group, even when poll() has already reaped the host.
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
        finally:
            (common.ROOT / 'join-token').unlink(missing_ok=True)
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

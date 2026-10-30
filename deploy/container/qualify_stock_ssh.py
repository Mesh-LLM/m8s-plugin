"""Local exact-image SSH refusal and bounded-failure checks; synthetic inputs, no provider API."""
import base64
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
import tempfile
import time
import uuid

if len(sys.argv) not in (2,4):
    raise SystemExit('Usage: qualify_stock_ssh.py IMAGE [cancel|deadline PRIVATE_EPHEMERAL_TARGET]')
image = sys.argv[1]
mode = sys.argv[2] if len(sys.argv)>2 else 'refusal'
if mode not in ('refusal','cancel','deadline'): raise SystemExit('Unknown check')
if not re.fullmatch(r'(sha256:[a-f0-9]{64}|[^\s]+@sha256:[a-f0-9]{64})', image):
    raise SystemExit('Select the exact local image ID or immutable registry digest')
root = Path(tempfile.mkdtemp(prefix='m8s-stock-ssh-'))
os.chmod(root, 0o700)
name = 'm8s-stock-ssh-' + uuid.uuid4().hex[:12]
network = name + '-isolated'
result = {'passed': False, 'image': image, 'provider_calls': False, 'check': mode,
          'scope': 'Local isolated-image SSH and supervisor check; no GPU inference or provider destruction proof'}

def run(args, **kwargs):
    return subprocess.run(args, stdin=kwargs.pop('stdin', subprocess.DEVNULL),
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=kwargs.pop('timeout', 30), **kwargs)

def checked(args, **kwargs):
    p = run(args, **kwargs)
    if p.returncode:
        (root/'command-error.txt').write_bytes(p.stderr[:16384])
        raise RuntimeError('Local qualification command failed: ' + ' '.join(args[:3]))
    return p.stdout

def exists(file):
    return run(['docker', 'exec', name, 'test', '-e', '/var/lib/m8s/' + file]).returncode == 0

try:
    checked(['ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-f', str(root/'client')])
    pub = ' '.join((root/'client.pub').read_text().split()[:2])
    checked(['docker', 'network', 'create', '--internal', network])
    checked(['docker', 'run', '-d', '--platform', 'linux/amd64', '--name', name,
             '--network', network,
             '-e', 'M8S_PROVISION_SSH_KEY='+pub, '-e', 'CONTAINER_ID=424242',
             '-e', 'CONTAINER_API_KEY=synthetic-instance-only', image], timeout=60)
    port = '2222'
    host = None
    for _ in range(60):
        logs = checked(['docker', 'logs', name]).decode()
        if len(logs) > 65536:
            raise RuntimeError('Oversized container output')
        for line in logs.splitlines():
            if line.startswith('M8S_PROVISION_HOST_KEY_V1 '):
                host = line.removeprefix('M8S_PROVISION_HOST_KEY_V1 ')
        if host:
            break
        time.sleep(0.25)
    if not host:
        raise RuntimeError('Missing container host identity')
    (root/'known-hosts').write_text('['+name+']:'+port+' '+host+'\n')
    # Deliberately pin the unrelated client public key as the expected host.
    (root/'wrong-hosts').write_text('['+name+']:'+port+' '+pub+'\n')
    def ssh(known, command, frame=None):
        args = ['docker', 'run', '--rm', '-i', '--name', name+'-client', '--platform', 'linux/amd64', '--network', network, '--mount', 'type=bind,src='+str(root)+',dst=/proof,readonly', '--entrypoint', '/usr/bin/ssh', image, '-F', '/dev/null', '-o', 'BatchMode=yes', '-o', 'IdentitiesOnly=yes',
                '-o', 'StrictHostKeyChecking=yes', '-o', 'GlobalKnownHostsFile=/dev/null',
                '-o', 'UserKnownHostsFile=/proof/'+known, '-o', 'ConnectTimeout=5',
                '-i', '/proof/client', '-p', port, 'm8s@'+name, command]
        if mode != 'refusal' and command == 'join-v0' and known == 'known-hosts' and not exists('provision-consumed'):
            child = subprocess.Popen(args,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            child.stdin.write(frame); child.stdin.close(); child.stdin=None
            probe = "from pathlib import Path\nimport json\npids={}\nfor p in Path('/proc').glob('[0-9]*/cmdline'):\n try: a=p.read_bytes().split(b'\\0')\n except FileNotFoundError: continue\n if b'/opt/mesh/mesh-llm' in a: pids['worker']=int(p.parent.name)\n if b'/usr/local/lib/m8s/token_provision.py' in a and b'receive' in a: pids['supervisor']=int(p.parent.name)\nprint(json.dumps(pids))"
            try:
                for _ in range(30):
                    pids=json.loads(checked(['docker','exec',name,'python3','-c',probe]))
                    if 'worker' in pids and 'supervisor' in pids: break
                    if child.poll() is not None: break
                    time.sleep(0.1)
                result['worker_observed_running']= 'worker' in pids and 'supervisor' in pids
                if mode == 'cancel' and result['worker_observed_running']:
                    checked(['docker','exec',name,'kill','-TERM',str(pids['supervisor'])])
                    result['supervisor_signalled']=True
                out,err=child.communicate(timeout=12)
                return subprocess.CompletedProcess(args,child.returncode,out,err)
            finally:
                if child.poll() is None: child.kill(); child.wait()
        return subprocess.run(args, input=frame, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15)
    wrong = ssh('wrong-hosts', 'join-v0', b'not-a-token')
    result['wrong_host_rejected_before_consumption'] = wrong.returncode == 255 and not exists('provision-consumed')
    denied = ssh('known-hosts', 'uname', b'not-a-token')
    result['unapproved_command_rejected_before_consumption'] = denied.returncode != 0 and not exists('provision-consumed')
    config = {'schema':'m8s.token-join/v0','instance_id':424242,'image_sha256':'a'*64,
              'model':'unsloth/Qwen3.8-27B-GGUF@4ca720788d1e01f1bff70c033e0d0028fd02e502/Qwen3.8-27B-UD-Q4_K_M.gguf',
              'mesh_id':'b'*32,'lifetime_seconds':3}
    data = json.dumps(config, separators=(',',':')).encode()
    token = b'synthetic-invalid-invite'
    frame = b'M8S-TOKEN-JOIN-V0\n'+struct.pack('>I',len(data))+data+struct.pack('>I',len(token))+token
    if mode != 'refusal':
        target = json.loads(Path(sys.argv[3]).read_text())
        config['mesh_id'] = target['mesh_id']
        config['lifetime_seconds'] = 60 if mode == 'cancel' else 5
        data = json.dumps(config,separators=(',',':')).encode()
        token = Path(target['join_file']).read_bytes()
        frame = b'M8S-TOKEN-JOIN-V0\n'+struct.pack('>I',len(data))+data+struct.pack('>I',len(token))+token
    started = time.monotonic()
    first = ssh('known-hosts', 'join-v0', frame)
    result['elapsed_seconds']=time.monotonic()-started
    result['bounded_refusal_and_token_erasure'] = first.returncode != 0 and time.monotonic()-started < 12 and exists('provision-consumed') and not exists('join-token')
    result['no_readiness_or_token_output'] = not first.stdout and token not in first.stderr
    again = ssh('known-hosts', 'join-v0', frame)
    result['replay_rejected'] = again.returncode != 0 and not again.stdout and not exists('join-token')
    for _ in range(20):
        p = run(['docker','exec',name,'python3','-c',
                 "from pathlib import Path\nimport sys\ntry:\n for p in Path('/proc').glob('[0-9]*/cmdline'):\n  try: data=p.read_bytes()\n  except FileNotFoundError: continue\n  if b'/opt/mesh/mesh-llm' in data.split(b'\\0'): sys.exit(0)\nexcept Exception: sys.exit(2)\nsys.exit(1)"])
        if p.returncode == 1:
            result['worker_process_absent'] = True
            break
        time.sleep(0.1)
    result['passed'] = all(result.get(k) is True for k in ['wrong_host_rejected_before_consumption','unapproved_command_rejected_before_consumption','bounded_refusal_and_token_erasure','no_readiness_or_token_output','replay_rejected','worker_process_absent'])
    if mode != 'refusal':
        result['passed'] = result['passed'] and result.get('worker_observed_running') is True
        if mode == 'cancel': result['passed'] = result['passed'] and result.get('supervisor_signalled') is True
        else: result['passed'] = result['passed'] and result['elapsed_seconds'] >= config['lifetime_seconds']
except Exception as error:
    result['error'] = str(error)
finally:
    run(['docker','rm','-f',name+'-client'])
    (root/'container-log.txt').write_bytes(run(['docker','logs',name]).stdout)
    (root/'container-state.txt').write_bytes(run(['docker','inspect','--format','{{json .State}} {{json .NetworkSettings.Ports}} {{json .HostConfig.PortBindings}}',name]).stdout)
    removed = run(['docker','rm','-f',name])
    result['local_container_removed'] = removed.returncode == 0
    result['local_network_removed'] = run(['docker','network','rm',network]).returncode == 0
    if result['local_container_removed']:
        (root/'client').unlink(missing_ok=True)
    result['temporary_client_key_removed'] = not (root/'client').exists()
    result['passed'] = result['passed'] and result['local_container_removed'] and result['local_network_removed'] and result['temporary_client_key_removed']
    (root/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    os.chmod(root/'result.json',0o600)
    print(json.dumps({**result,'private_evidence_directory':str(root)},indent=2))
    sys.exit(0 if result['passed'] else 1)

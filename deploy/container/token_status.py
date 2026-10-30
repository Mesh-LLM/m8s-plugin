"""Bounded, secret-free observations; never admission or inference authority."""
import json
import os
from pathlib import Path
import stat

SCHEMA = 'm8s.token-worker-status/v0'
PHASES = {'awaiting_join', 'unknown', 'starting', 'waiting_for_mesh', 'joined', 'stopped'}
COUNTERS = {'download_restart_count', 'elapsed_seconds', 'cache_file_bytes', 'cache_allocated_bytes',
            'cache_file_count', 'cache_incomplete_files', 'worker_rss_bytes',
            'worker_read_bytes', 'worker_write_bytes', 'network_received_bytes',
            'network_sent_bytes', 'requested_model_count', 'hosted_model_count',
            'runtime_model_count', 'runtime_instance_count', 'runtime_error_count'}
# Heuristic keyword matches, never raw errors or admission/readiness evidence.
ERROR_HINTS = {
    'runtime_error_download': ('download', 'reconstruction', 'cas request', 'xet'),
    'runtime_error_timeout': ('timeout', 'timed out'),
    'runtime_error_connection': ('connection', 'connect error', 'dns', 'broken pipe'),
    'runtime_error_auth': ('unauthorized', 'forbidden', '401', '403'),
    'runtime_error_rate_limit': ('rate limit', '429'),
    'runtime_error_disk': ('no space left', 'disk quota', 'os error 28'),
    'runtime_error_memory': ('out of memory', 'allocation failed'),
    'runtime_error_native': ('cuda', 'native runtime', 'abi mismatch', 'shared librar'),
}
FLAGS = set(ERROR_HINTS) | {'worker_running', 'api_reachable', 'mesh_matches', 'cache_scan_truncated',
         'runtime_snapshot_truncated', 'node_loading', 'node_serving'}


def project(value):
    if type(value) is not dict or value.get('schema') != SCHEMA or value.get('phase') not in PHASES:
        raise ValueError('Invalid worker observation')
    result = {'schema': SCHEMA, 'phase': value['phase']}
    for key in COUNTERS:
        v = value.get(key)
        if type(v) is int and 0 <= v < 2**64:
            result[key] = v
    for key in FLAGS:
        if type(value.get(key)) is bool:
            result[key] = value[key]
    code = value.get('worker_exit_code')
    if type(code) is int and -255 <= code <= 255:
        result['worker_exit_code'] = code
    return result


def runtime_metrics(status):
    """Project counts/known states only; never return error text or model paths."""
    if type(status) is not dict:
        return {}
    result = {}
    truncated = False
    runtime = status.get('runtime')
    runtime = runtime if type(runtime) is dict else {}
    summary = runtime.get('intent_summary')
    summary = summary if type(summary) is dict else {}
    for key, value, expected in [
        ('requested_model_count', status.get('requested_models'), str),
        ('hosted_model_count', status.get('hosted_models'), str),
        ('runtime_model_count', runtime.get('models'), dict),
        ('runtime_instance_count', runtime.get('lifecycle_instances'), dict),
        ('runtime_error_count', summary.get('recent_errors'), str),
    ]:
        if type(value) is list:
            truncated |= len(value) > 256
            result[key] = sum(type(item) is expected and bool(item) for item in value[:256])
    errors = summary.get('recent_errors')
    if type(errors) is list:
        texts = [e[:4096].lower() for e in errors[:256] if type(e) is str]
        for key, patterns in ERROR_HINTS.items():
            result[key] = any(pattern in text for text in texts for pattern in patterns)
    if result:
        result['runtime_snapshot_truncated'] = truncated
    if status.get('node_state') in ('client', 'standby', 'loading', 'serving', 'worker'):
        result['node_loading'] = status['node_state'] == 'loading'
        result['node_serving'] = status['node_state'] == 'serving'
    return result


def read(root):
    path = root / 'worker-status.json'
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    except FileNotFoundError:
        return {'schema': SCHEMA, 'phase': 'unknown' if (root / 'provision-consumed').exists() else 'awaiting_join'}
    with os.fdopen(fd, 'rb') as stream:
        m = os.fstat(stream.fileno())
        if not (stat.S_ISREG(m.st_mode) and m.st_uid == os.geteuid()
                and stat.S_IMODE(m.st_mode) == 0o600 and m.st_nlink == 1 and m.st_size <= 8192):
            raise ValueError('Invalid worker observation')
        data = stream.read(8193)
    if len(data) > 8192:
        raise ValueError('Invalid worker observation')
    return project(json.loads(data))


def write(root, value):
    data = json.dumps(project(value), separators=(',', ':')).encode()
    temp = root / ('.worker-status-' + str(os.getpid()) + '.tmp')
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp, root / 'worker-status.json')
        directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        temp.unlink(missing_ok=True)


def cache_metrics(root, limit=512):
    result = {'cache_file_bytes': 0, 'cache_allocated_bytes': 0,
              'cache_file_count': 0, 'cache_incomplete_files': 0, 'cache_scan_truncated': False}
    pending = [root / 'hf-cache']
    seen = 0
    while pending:
        path = pending.pop()
        try:
            if path.is_symlink():
                continue
            with os.scandir(path) as entries:
                for entry in entries:
                    seen += 1
                    if seen > limit:
                        result['cache_scan_truncated'] = True
                        return result
                    if entry.is_dir(follow_symlinks=False):
                        pending.append(Path(entry.path))
                    elif entry.is_file(follow_symlinks=False):
                        m = entry.stat(follow_symlinks=False)
                        result['cache_file_count'] += 1
                        result['cache_file_bytes'] += m.st_size
                        result['cache_allocated_bytes'] += m.st_blocks * 512
                        result['cache_incomplete_files'] += int(entry.name.endswith('.incomplete'))
        except FileNotFoundError:
            pass
        except OSError:
            result['cache_scan_truncated'] = True
    return result


def process_metrics(pid):
    result = {}
    if pid is None:
        return result
    try:
        pages = int(Path('/proc', str(pid), 'statm').read_text().split()[1])
        result['worker_rss_bytes'] = pages * os.sysconf('SC_PAGE_SIZE')
        for line in Path('/proc', str(pid), 'io').read_text().splitlines():
            key, value = line.split(':', 1)
            if key in ('read_bytes', 'write_bytes'):
                result['worker_' + key] = int(value)
    except (OSError, ValueError, IndexError):
        pass
    return result


def network_metrics():
    result = {}
    try:
        received, sent = 0, 0
        for line in Path('/proc/net/dev').read_text().splitlines()[2:]:
            name, fields = line.split(':', 1)
            if name.strip() != 'lo':
                values = fields.split()
                received += int(values[0])
                sent += int(values[8])
        result = {'network_received_bytes': received, 'network_sent_bytes': sent}
    except (OSError, ValueError, IndexError):
        pass
    return result


def snapshot(root, phase, elapsed, child, api_reachable=False, mesh_matches=False, runtime=None, download_restarts=0):
    value = {'schema': SCHEMA, 'phase': phase, 'download_restart_count': download_restarts, 'elapsed_seconds': max(0, int(elapsed)),
             'worker_running': phase != 'stopped' and child is not None and child.poll() is None,
             'api_reachable': api_reachable, 'mesh_matches': mesh_matches}
    if child is not None:
        value['worker_exit_code'] = child.returncode
    value.update(runtime_metrics(runtime))
    value.update(cache_metrics(root))
    value.update(process_metrics(child.pid if child is not None else None))
    value.update(network_metrics())
    write(root, value)

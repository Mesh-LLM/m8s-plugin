import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import token_status as status
import token_provision as worker


class TokenStatusTests(unittest.TestCase):
    def test_projection_cannot_return_private_fields_or_strings_as_metrics(self):
        value = {'schema': status.SCHEMA, 'phase': 'starting', 'worker_running': True,
                 'elapsed_seconds': 7, 'cache_file_bytes': 'private-token',
                 'node_key': 'private-key', 'join_token': 'private-invite',
                 'stderr': 'private-logs', 'worker_exit_code': -15}
        result = status.project(value)
        self.assertEqual(result['elapsed_seconds'], 7)
        self.assertEqual(result['worker_exit_code'], -15)
        self.assertNotIn('private', json.dumps(result))
        for phase in ['private-invite', '', None]:
            with self.assertRaises(ValueError):
                status.project({**value, 'phase': phase})

    def test_runtime_projection_is_bounded_and_never_returns_private_error_text(self):
        value = {'node_state': 'loading', 'requested_models': ['private/model'],
                 'hosted_models': [], 'token': 'private-invite',
                 'runtime': {'models': [{'name': '/private/model'}],
                             'lifecycle_instances': [{'state': 'private-state'}],
                             'intent_summary': {'recent_errors': ['private-token-in-error'] * 300}}}
        result = status.runtime_metrics(value)
        self.assertEqual(result['runtime_error_count'], 256)
        self.assertEqual(result['requested_model_count'], 1)
        self.assertEqual(result['hosted_model_count'], 0)
        self.assertTrue(result['node_loading'])
        self.assertTrue(result['runtime_snapshot_truncated'])
        self.assertNotIn('private', json.dumps(result))
        self.assertEqual(status.runtime_metrics({'runtime': None}), {})
        self.assertEqual(status.runtime_metrics([]), {})
        self.assertEqual(status.runtime_metrics({'runtime': {'intent_summary': {'recent_errors': 'private'}}}), {})
        self.assertNotIn('node_loading', status.runtime_metrics({'node_state': 'private'}))
        projected = status.project({'schema': status.SCHEMA, 'phase': 'joined', **result})
        self.assertEqual(projected['runtime_error_count'], 256)

    def test_private_atomic_snapshot_and_file_refusals(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            value = {'schema': status.SCHEMA, 'phase': 'stopped', 'worker_running': False}
            status.write(root, value)
            self.assertEqual(status.read(root), value)
            self.assertEqual(list(root.glob('*.tmp')), [])
            path = root / 'worker-status.json'
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            path.chmod(0o644)
            with self.assertRaises(ValueError): status.read(root)
            path.unlink()
            private = root / 'join-token'
            private.write_text('private-invite')
            path.symlink_to(private)
            with self.assertRaises(OSError): status.read(root)
            path.unlink()
            path.write_text('x' * 8193); path.chmod(0o600)
            with self.assertRaises(ValueError): status.read(root)

    def test_cache_scan_is_bounded_and_does_not_follow_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); cache = root / 'hf-cache'; cache.mkdir()
            (root / 'private').write_bytes(b'secret' * 100)
            (cache / 'outside').symlink_to(root / 'private')
            (cache / 'model.incomplete').write_bytes(b'x' * 17)
            result = status.cache_metrics(root)
            self.assertEqual(result['cache_file_bytes'], 17)
            self.assertEqual(result['cache_incomplete_files'], 1)
            self.assertNotIn('model', json.dumps(result))
            self.assertTrue(status.cache_metrics(root, limit=1)['cache_scan_truncated'])

    def test_status_command_does_not_consume_provisioning_or_read_invite(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); (root / 'join-token').write_text('private-invite')
            output = io.StringIO()
            with patch.object(worker.common, 'ROOT', root), \
                 patch.object(worker.common, 'UID', os.geteuid()), \
                 patch.dict(os.environ, {'SSH_ORIGINAL_COMMAND': 'status-v0'}), \
                 patch.object(sys, 'stdout', output), \
                 patch.object(worker.subprocess, 'Popen') as spawn:
                worker.receive()
            spawn.assert_not_called()
            self.assertEqual(json.loads(output.getvalue())['phase'], 'awaiting_join')
            self.assertFalse((root / 'provision-consumed').exists())
            self.assertEqual((root / 'join-token').read_text(), 'private-invite')
            self.assertNotIn('private', output.getvalue())
            (root / 'provision-consumed').touch()
            self.assertEqual(status.read(root)['phase'], 'unknown')


if __name__ == '__main__':
    unittest.main()

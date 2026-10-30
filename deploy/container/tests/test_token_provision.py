import importlib.util
import io
import json
from pathlib import Path
import struct
import sys
import unittest
from unittest.mock import patch, Mock, MagicMock
import tempfile
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import token_provision as target

class TokenProvisionTests(unittest.TestCase):
    def config(self):
        return {'schema':'m8s.token-join/v0','instance_id':123,'image_sha256':'a'*64,
                'model':target.common.QWEN38_MODEL_REF,'mesh_id':'b'*16,'lifetime_seconds':60}
    def frame(self, config=None, token=b'synthetic-token'):
        data=json.dumps(config or self.config()).encode()
        return target.MAGIC+struct.pack('>I',len(data))+data+struct.pack('>I',len(token))+token
    def test_token_only_travels_in_private_frame(self):
        config,token=target.packet(io.BytesIO(self.frame()))
        self.assertEqual(token,b'synthetic-token')
        argv=target.command(config)
        self.assertIn('--join-file',argv)
        self.assertIn('--no-draft',argv)
        self.assertNotIn('synthetic-token',' '.join(argv))
        self.assertNotIn('--require-cert',argv)
        self.assertNotIn('--auto',argv)
    def test_rejects_truncation_trailing_data_and_token_injection(self):
        for frame in [self.frame()[:-1],self.frame()+b'x',self.frame(token=b'line\nbreak'),self.frame(token=b'')]:
            with self.assertRaises(ValueError):target.packet(io.BytesIO(frame))
    def test_bounds_runtime_and_refuses_extra_policy_fields(self):
        for key,value in [('lifetime_seconds',0),('lifetime_seconds',1501),('lifetime_seconds',True),('model','other-model'),('mesh_id','bad'),('disable_auth',True)]:
            config=self.config();config[key]=value
            with self.assertRaises(ValueError):target.packet(io.BytesIO(self.frame(config)))
    def test_start_failure_removes_private_join_file(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'instance-id').write_text('123')
            with patch.object(target.common, 'ROOT', root), \
                 patch.object(target.os, 'geteuid', return_value=target.common.UID), \
                 patch.dict(target.os.environ, {'SSH_ORIGINAL_COMMAND':'join-v0'}), \
                 patch.object(target.signal, 'alarm'), patch.object(target.signal, 'signal'), \
                 patch.object(target.sys, 'stdin', type('Input', (), {'buffer':io.BytesIO(self.frame())})()), \
                 patch.object(target.subprocess, 'Popen', side_effect=OSError('synthetic failure')):
                with self.assertRaises(OSError): target.receive()
            self.assertFalse((root / 'join-token').exists())
            self.assertTrue((root / 'provision-consumed').exists())
    def test_denied_admission_and_interruption_stop_worker_and_erase_token(self):
        for interrupted, already_exited in ((None, False), (target.signal.SIGTERM, False), (target.signal.SIGHUP, False), (None, True)):
            with self.subTest(interrupted=interrupted, already_exited=already_exited), tempfile.TemporaryDirectory() as directory:
                root=Path(directory);(root/'instance-id').write_text('123')
                config=self.config();config['lifetime_seconds']=2
                child=Mock(pid=456);child.poll.return_value=1 if already_exited else None
                handlers = {}
                def probe(*args, **kwargs):
                    if interrupted:
                        handlers[interrupted](interrupted, None)
                    raise OSError('synthetic denied/unready endpoint')
                opener=Mock();opener.open.side_effect=probe
                output=io.StringIO()
                with patch.object(target.common,'ROOT',root), \
                     patch.object(target.os,'geteuid',return_value=target.common.UID), \
                     patch.dict(target.os.environ,{'SSH_ORIGINAL_COMMAND':'join-v0','CONTAINER_API_KEY':'synthetic-instance-secret'}), \
                     patch.object(target.signal,'alarm'), patch.object(target.signal,'signal',side_effect=lambda sig, handler: handlers.update({sig: handler})), \
                     patch.object(target.sys,'stdin',type('Input',(),{'buffer':io.BytesIO(self.frame(config))})()), \
                     patch.object(target.sys,'stdout',output), \
                     patch.object(target.subprocess,'Popen',return_value=child) as spawn, \
                     patch.object(target.urllib.request,'build_opener',return_value=opener), \
                     patch.object(target.time,'monotonic',side_effect=[0,0.5,3]), \
                     patch.object(target.time,'sleep'), \
                     patch.object(target.os,'killpg') as kill:
                    with self.assertRaises(target.WorkerStopped if interrupted else ValueError):target.receive()
                self.assertEqual(spawn.call_args.kwargs['env']['MESH_HF_RETRY_MAX_ATTEMPTS'],'0')
                self.assertEqual(spawn.call_args.kwargs['env']['HF_XET_CLIENT_RETRY_MAX_ATTEMPTS'],'0')
                self.assertEqual(spawn.call_args.kwargs['env']['MESH_LLM_NO_DEFAULT_PLUGINS'],'1')
                self.assertEqual(spawn.call_args.kwargs['env']['LD_LIBRARY_PATH'],'/usr/local/nvidia/lib64:/usr/local/nvidia/lib')
                self.assertEqual(spawn.call_args.kwargs['env']['PATH'],'/usr/local/nvidia/bin:/usr/bin:/bin')
                self.assertNotIn('CONTAINER_API_KEY',spawn.call_args.kwargs['env'])
                self.assertEqual([c.args for c in kill.call_args_list], [(456,target.signal.SIGTERM),(456,target.signal.SIGKILL)])
                self.assertEqual(child.wait.call_count,2)
                self.assertEqual(child.wait.call_args_list[0].kwargs,{'timeout':3})
                self.assertFalse((root/'join-token').exists())
                self.assertEqual(output.getvalue(),'')
                self.assertTrue((root/'provision-consumed').exists())
    def test_download_recovery_is_once_and_keeps_original_deadline_and_identity(self):
        for changed_identity in (False, True):
            with self.subTest(changed_identity=changed_identity), tempfile.TemporaryDirectory() as directory:
                root = Path(directory); (root / 'instance-id').write_text('123')
                first, second = Mock(pid=456), Mock(pid=457)
                first.poll.return_value = second.poll.return_value = None
                status = {'node_id': 'a' * 10, 'mesh_id': 'b' * 16, 'node_state': 'standby',
                          'requested_models': ['pinned-model'], 'hosted_models': [],
                          'runtime': {'intent_summary': {'recent_errors': ['Xet download failed: connection error']}}}
                response = MagicMock()
                response.__enter__.return_value.read.side_effect = [
                    json.dumps(status).encode(),
                    json.dumps({**status, 'node_id': ('c' if changed_identity else 'a') * 10}).encode(),
                    json.dumps(status).encode()]
                opener = Mock(); opener.open.return_value = response
                output = io.StringIO()
                with patch.object(target.common, 'ROOT', root), \
                     patch.object(target.os, 'geteuid', return_value=target.common.UID), \
                     patch.dict(target.os.environ, {'SSH_ORIGINAL_COMMAND': 'join-v0'}), \
                     patch.object(target.signal, 'alarm'), patch.object(target.signal, 'signal'), \
                     patch.object(target.sys, 'stdin', type('Input', (), {'buffer': io.BytesIO(self.frame())})()), \
                     patch.object(target.sys, 'stdout', output), \
                     patch.object(target.subprocess, 'Popen', side_effect=[first, second]) as spawn, \
                     patch.object(target.urllib.request, 'build_opener', return_value=opener), \
                     patch.object(target.time, 'monotonic', side_effect=[0, 1, 2, 3, 4, 5, 61]), \
                     patch.object(target.time, 'sleep'), patch.object(target.os, 'killpg') as kill, \
                     patch.object(target.token_status, 'snapshot'):
                    if changed_identity:
                        with self.assertRaises(RuntimeError): target.receive()
                    else:
                        target.receive()
                self.assertEqual(spawn.call_count, 2)
                self.assertEqual(spawn.call_args_list[0], spawn.call_args_list[1])
                self.assertEqual(spawn.call_args.kwargs['env']['MESH_LLM_NODE_KEY_PATH'], str(root / 'node.key'))
                self.assertEqual([c.args[0] for c in kill.call_args_list], [456, 456, 457, 457])
                self.assertEqual(len(output.getvalue().splitlines()), 1)
                self.assertFalse((root / 'join-token').exists())

    def test_non_download_and_resource_errors_do_not_trigger_recovery(self):
        for error in ('CUDA load failed', 'download forbidden 403', 'download: no space left',
                      'download out of memory', 'unknown failure'):
            status = {'requested_models': ['model'], 'hosted_models': [], 'node_state': 'standby',
                      'runtime': {'intent_summary': {'recent_errors': [error]}}}
            self.assertFalse(target.recoverable_download(status))
        self.assertFalse(target.recoverable_download(None))

    def test_consumed_provisioning_is_not_replayed(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);(root/'provision-consumed').write_text('used')
            with patch.object(target.common,'ROOT',root), \
                 patch.object(target.os,'geteuid',return_value=target.common.UID), \
                 patch.dict(target.os.environ,{'SSH_ORIGINAL_COMMAND':'join-v0'}), \
                 patch.object(target.signal,'alarm'), \
                 patch.object(target.subprocess,'Popen') as spawn:
                with self.assertRaises(FileExistsError):target.receive()
            spawn.assert_not_called()
    def test_stock_main_random_mesh_id(self):
        config=self.config();config['mesh_id']='b'*32
        parsed,_=target.packet(io.BytesIO(self.frame(config)))
        self.assertEqual(parsed['mesh_id'],'b'*32)
    def test_certified_frame_is_not_a_token_frame(self):
        with self.assertRaises(ValueError):target.packet(io.BytesIO(target.common.MAGIC+self.frame()[len(target.MAGIC):]))

if __name__=='__main__':unittest.main()

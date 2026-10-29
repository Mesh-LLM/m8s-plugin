import importlib.util
import io
import json
from pathlib import Path
import struct
import sys
import unittest
from unittest.mock import patch, Mock
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
        for interrupted in (False, True):
            with self.subTest(interrupted=interrupted), tempfile.TemporaryDirectory() as directory:
                root=Path(directory);(root/'instance-id').write_text('123')
                config=self.config();config['lifetime_seconds']=2
                child=Mock(pid=456);child.poll.return_value=None
                opener=Mock();opener.open.side_effect=OSError('synthetic denied/unready endpoint')
                output=io.StringIO()
                with patch.object(target.common,'ROOT',root), \
                     patch.object(target.os,'geteuid',return_value=target.common.UID), \
                     patch.dict(target.os.environ,{'SSH_ORIGINAL_COMMAND':'join-v0'}), \
                     patch.object(target.signal,'alarm'), patch.object(target.signal,'signal'), \
                     patch.object(target.sys,'stdin',type('Input',(),{'buffer':io.BytesIO(self.frame(config))})()), \
                     patch.object(target.sys,'stdout',output), \
                     patch.object(target.subprocess,'Popen',return_value=child) as spawn, \
                     patch.object(target.urllib.request,'build_opener',return_value=opener), \
                     patch.object(target.time,'monotonic',side_effect=[0,0.5,3]), \
                     patch.object(target.time,'sleep',side_effect=InterruptedError() if interrupted else None), \
                     patch.object(target.os,'killpg') as kill:
                    with self.assertRaises(InterruptedError if interrupted else ValueError):target.receive()
                self.assertEqual(spawn.call_args.kwargs['env']['MESH_HF_RETRY_MAX_ATTEMPTS'],'0')
                self.assertEqual(spawn.call_args.kwargs['env']['HF_XET_CLIENT_RETRY_MAX_ATTEMPTS'],'0')
                kill.assert_called_once_with(456,target.signal.SIGTERM)
                child.wait.assert_called_once_with(timeout=3)
                self.assertFalse((root/'join-token').exists())
                self.assertEqual(output.getvalue(),'')
                self.assertTrue((root/'provision-consumed').exists())
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

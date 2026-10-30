"""Public artifact contract: canonical HF metadata, never an operator receipt."""
import hashlib
import json
import pathlib
import unittest

class QwenIdentity(unittest.TestCase):
    def test_native_identity_hashes_canonical_hf_source_not_raw_blob(self):
        artifact = json.loads((pathlib.Path(__file__).parents[1] / 'qwen38-model-artifact.json').read_text())
        # Native HuggingFaceGgufIdentity field order is part of this wire hash.
        canonical = {'schema_version': 1, 'source': 'hugging-face-gguf',
                     'repository': artifact['model'],
                     'revision': '4ca720788d1e01f1bff70c033e0d0028fd02e502',
                     'files': [{'path': 'Qwen3.8-27B-UD-Q4_K_M.gguf',
                                'bytes': artifact['bytes'], 'sha256': artifact['sha256']}]}
        digest = hashlib.sha256(json.dumps(canonical, separators=(',', ':')).encode()).hexdigest()
        self.assertEqual(digest, '7756e8943d5ec98b1cd76895d33d20b8c8bf7609a71540fc9b6fa512bdedd0de')
        self.assertNotEqual(digest, artifact['sha256'])
        model_source = (pathlib.Path(__file__).parents[3] / 'crates/m8s-cli/src/prototype/demo_model.rs').read_text()
        self.assertIn('local-gguf/sha256-' + digest, model_source)

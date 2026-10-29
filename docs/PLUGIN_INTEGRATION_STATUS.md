# Plugin integration checkpoint — 2026-10-29

The standalone plugin adapter builds against Mesh f2a7f62c and passes strict
all-target Clippy. It exposes read-only status using the existing dashboard's
socket-path and peer-UID checks. CLI/daemon rental behavior is unchanged.

## Validation

- Plugin cargo check, cargo build, strict all-target Clippy: passed.
- Plugin and controller formatting: passed.
- Controller scripts/check --no-pty --no-acl-mutation: failed in existing
  m8s-agent tests (47 passed, 13 failed). Failures cover synthetic container
  lifecycle fixtures and native-ID/material fixtures. No source in that crate
  changed. Root cause remains unresolved; do not claim the full gate passed.
- Plugin host IPC handshake and real controller observation: not yet qualified.
- UI install: blocked by registry.npmjs.org certificate-chain verification in
  npm and curl. System trust-store attempts did not resolve it. No TLS bypass.
- No new paid rentals performed for this integration.

## Required next acceptance

1. Authenticated attachment to selected local node and owner, whether passive
   client or inference host. Bind fleet authorization to owner/node/mesh.
2. Resolve controller test failures and qualify plugin IPC with real m8sd.
3. Complete offer quote/launch/release API and expose it to the desktop.
4. Complete desktop build/lint/browser checks and inspect rendered layouts.
5. Live Vast rental, verified inference and exact confirmed destruction, followed
   by a recording of that actual workflow. Mocked UI is not live acceptance.

Publication should preserve the private source repository and import reviewed
source into Mesh-LLM/m8s-plugin without exporting private historical revisions.

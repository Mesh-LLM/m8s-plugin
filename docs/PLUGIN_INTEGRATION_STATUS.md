# Plugin integration checkpoint — 2026-10-29

The standalone plugin adapter builds against Mesh f2a7f62c and passes strict
all-target Clippy. It exposes read-only status using the existing dashboard's
socket-path and peer-UID checks. CLI/daemon rental behavior is unchanged.

## Validation

- Plugin cargo check, cargo build, strict all-target Clippy: passed.
- Plugin and controller formatting: passed.
- Controller `RUST_TEST_THREADS=1 scripts/check --no-pty --no-acl-mutation`: passed through Terminal: 826 tests passed, 5 ignored, 7 filtered; strict Clippy passed. The initial concurrent/native failures did not recur in the serial run. Restricted-shell runs also encountered explicit socket EPERM.
- Plugin host IPC handshake and real controller observation: not yet qualified.
- UI build/lint/formatting and 31 mocked browser tests passed, followed by four focused compute tests. Dependencies came from the public Yarn mirror in a temporary install with original lockfile integrity; TLS verification remained enabled.
- No new paid rentals performed for this integration.

## Required next acceptance

1. Authenticated attachment to selected local node and owner, whether passive
   client or inference host. Bind fleet authorization to owner/node/mesh.
2. Qualify plugin IPC with real m8sd; investigate concurrent test sensitivity separately.
3. Complete offer quote/launch/release API and expose it to the desktop.
4. Complete desktop build/lint/browser checks and inspect rendered layouts.
5. Live Vast rental, verified inference and exact confirmed destruction, followed
   by a recording of that actual workflow. Mocked UI is not live acceptance.

Publication should preserve the private source repository and import reviewed
source into Mesh-LLM/m8s-plugin without exporting private historical revisions.

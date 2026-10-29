# v0 validation checkpoint

This is a sanitized source-validation summary, not a paid rental receipt.
Stock GPU inference and the replacement desktop recording remain unverified.

## Verified locally

- Built unmodified upstream main `2809443345ea8134db62a8b433f5bb2bb5f1d123`
  with `just mesh` using Rust 1.95. The host reports `0.78.0+g280944`.
  The upstream worktree remained clean. This builds the host and console, not
  the required Linux GPU native runtime.
- Started an isolated, model-free unpublished Mesh with a fresh local identity
  and the actual m8s plugin executable. Mesh registered `m8s.compute` through
  its plugin IPC handshake. MCP initialization, tool discovery and `compute`
  with `{"operation":"status"}` passed through the running Mesh host.
- That call returned `m8s.operator/v0`, `certificate_verified: false`, and an
  unconsumed attempt bound to the selected mesh. Main's 32-character mesh IDs
  are accepted. The smoke test made no provider calls, created no rentals and
  confirmed its isolated host stopped afterward.
- The root filtered suite passed (seven PTY/ACL tests excluded), followed by
  focused checks for the final token changes and strict workspace Clippy.
  The plugin's three tests and strict Clippy passed; 44 container tests passed, including interruption/denial cleanup and
  refusal to replay consumed provisioning.
- The companion app's Rust tests and strict Clippy passed. UI lint/build and
  all 48 mocked browser tests passed. Recorder/editor syntax checks passed.
  These are implementation checks, not GPU or real-conversation acceptance.

- Two isolated stock-main passive nodes with independent identities joined the
  same mesh through a private join file, each observed one peer, and both stopped
  cleanly. This verifies local token admission, not remote GPU inference or
  compatibility with the app's older embedded runtime.
- The actual stock-main binary passed synthetic HTTP 503 probes at the Xet CAS
  reconstruction and payload endpoints: each distinct failed range was requested
  once, and download failed without a retry. Reproduce with
  `node deploy/container/qualify_stock_download.mjs /absolute/mesh-llm cas`
  and the same command ending in `payload`. These use local synthetic services,
  isolated caches and no provider credentials or real weights.
- The worker sets both upstream retry controls (`MESH_HF_RETRY_MAX_ATTEMPTS`
  and `HF_XET_CLIENT_RETRY_MAX_ATTEMPTS`) to zero and limits inference to one
  slot, 4096 context tokens and no draft model. These are not a global no-retry
  guarantee: a separate pinned-reference Hub metadata failure probe entered a
  fallback resolver that repeated requests. The Xet assertions above deliberately
  exclude metadata lookup behavior. Full-image size and real transfer accounting
  still require qualification; the transfer reserve and cleanup remain mandatory.

## Still required

1. Build and qualify a matching stock-main Linux CUDA/SM120 product and its
   immutable worker image, including native download transfer bounds. The
   downloaded official v0.78.1 product has ABI 0.1.66; main requires 0.1.74.
   Its source is outside the checked main ancestry. Do not edit metadata to
   force acceptance or substitute the historical private fork.
2. Verify the exact image's admission failure, host-key failure, cancellation,
   bounded shutdown and provider cleanup behavior.
3. Run one freshly authorized bounded rental through the shared plugin/app
   workflow, complete inference through the selected mesh and a real app
   conversation, then independently confirm exact-resource destruction.
4. Review and replace the demo only after successful capture. The current
   README cover deliberately says architecture preview and qualification pending.

The app still embeds upstream `b4b33ef8` (reports `0.72.1`). Its rebuilt headless
backend passed a local test with stock main: the app created a private passive
mesh, the main node joined through its token, each observed one peer, and the app
returned token-only status through the real shared plugin adapter. Both processes
stopped afterward, with no provider calls or rentals. This establishes local
admission and adapter interoperability, not GPU inference or the live demo.

The first Linux CUDA build completed native compilation but failed during
packaging because its builder lacked `patchelf`. The repaired builder includes
that dependency and preserves its filesystem across failures. The product build
and image qualification are still pending.
Raw logs, identities, captures and accounting evidence remain private.

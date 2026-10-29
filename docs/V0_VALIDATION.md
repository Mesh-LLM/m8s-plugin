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
  The plugin's three tests and strict Clippy passed; 42 container tests passed.
- The companion app's Rust tests and strict Clippy passed. UI lint/build and
  all 48 mocked browser tests passed. Recorder/editor syntax checks passed.
  These are implementation checks, not GPU or real-conversation acceptance.

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

The app still embeds upstream `b4b33ef8`. A standalone main host handshake does
not establish compatibility of that embedded runtime with the new worker.
Raw logs, identities, captures and accounting evidence remain private.

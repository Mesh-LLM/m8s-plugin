# m8s Mesh plugin

Initial read-only adapter to the existing m8sd controller. The `status` tool
uses the operator dashboard's socket path and peer-UID checks and bounded
three-second observation. It reports controller state, not independently
verified provider state. No provider secrets enter the plugin protocol.

Build with `cargo build --manifest-path plugin/Cargo.toml -j 2`.
Set `M8S_CONTROL_SOCKET` in the host environment to the explicitly selected
owner controller socket. Run `m8s-plugin --print-package-manifest` to emit
installer metadata; the normal invocation participates in Mesh's plugin IPC.

The plugin has its own Cargo workspace so the pinned Mesh SDK does not inflate
normal controller builds. Unix only, matching the current m8s control transport.

Package with `sh plugin/package.sh`, then install the generated archive using
`mesh-llm plugins install --archive plugin/target/m8s-0.1.0.tar.gz --name m8s --version 0.1.0`.

Outstanding: live host handshake qualification, mutation
capabilities with durable operation receipts, quote/offer API and console page.
The existing CLI remains the authoritative rental interface until these land.

Validation on 2026-10-29: pinned SDK compile, strict all-target Clippy and
binary build passed. Package metadata output is empty because this initial
adapter declares no static configuration or UI assets; MCP registration happens
at runtime. Live IPC/host registration remains unqualified.

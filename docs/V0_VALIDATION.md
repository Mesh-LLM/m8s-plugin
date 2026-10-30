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
- The full root `scripts/check` passed, including the seven synthetic PTY/ACL
  tests previously excluded. Formatting and strict workspace Clippy passed too.
  The plugin's five tests and strict Clippy passed; 45 container tests passed,
  including interruption/denial cleanup and
  refusal to replay consumed provisioning and process-group cleanup after the
  host exits before its children.
- Built the release plugin archive with `plugin/package.sh`. Stock main installed
  it through `plugins install --archive`, discovered `m8s.compute` from the local
  plugin store, and completed the MCP status call. No command-path override was
  used for this installed-package check; the isolated host stopped afterward.
  A further installed-package MCP check passed live `status`, `offers` and
  `quote` operations through stock main. The quote was bound to that main node
  and within the selected budget; no launch intent or rental was created.
- The shared quote operation now estimates compute, storage, the 64 GB transfer
  reserve, cleanup and contingency before returning a booking quote. Tests cover
  lifetime/monthly/hourly limits, missing prices, expiry and insufficient cleanup;
  refused quotes create neither a quote file nor launch intent. Controller budget
  admission remains independent and authoritative at launch.
- A read-only live Vast check through the rebuilt app and release plugin returned
  offers and a budget-qualified quote bound to the app's selected mesh. No launch
  intent or rental was created. This expired observation is not a price promise
  or permission to reuse the quote; a live run needs a new quote and inventory.
- Provider status now projects credential-file configuration from the plugin,
  without returning the key or path. The app uses this to enable browsing and
  identifies externally managed setup instead of offering unsupported key edits.
  Tests cover missing, writable and linked files and both browser states.
  The rebuilt app also passed a real, unmocked browser check: provider status
  came from the release plugin, Add compute displayed live offers, and a
  budget-qualified quote was bound to the selected mesh. No launch intent or
  rental was created; the isolated app stopped afterward.
- The companion app's Rust tests and strict Clippy passed. UI lint/build and
  all 50 mocked browser tests passed. Recorder/editor syntax checks passed.
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

1. Publish the locally checked stock-main Linux CUDA/SM120 worker and verify
   anonymous access to its immutable digest. Registry upload is currently blocked
   by missing GitHub package-writing scope. The
   downloaded official v0.78.1 product has ABI 0.1.66; main requires 0.1.74.
   Its source is outside the checked main ancestry. Do not edit metadata to
   force acceptance or substitute the historical private fork.
2. Complete remote admission/inference and provider cleanup qualification. Local
   image SSH refusal, cancellation and bounded shutdown checks passed as below.
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
A second check passed with an unsigned stock-main node, matching the remote
worker configuration: it joined without an owner certificate, both peers observed
each other, and both processes stopped. This confirms the weaker token-only
admission behavior locally; it does not establish ownership certification.

The first Linux CUDA build completed native compilation but failed during
packaging because its builder lacked `patchelf`. The repaired builder includes
that dependency and preserves its filesystem across failures. A subsequent
packaging attempt rejected `libnccl.so.2`: the CUDA devel image installed NCCL,
which the native build auto-enabled, while upstream's runtime packaging policy
allows only its reviewed CUDA redistributables. The next build uses upstream's
existing `-DGGML_CUDA_NCCL=OFF` CMake option for this single-GPU profile, followed
by the unchanged packaging recipe. No source or dependency-policy patch is
applied. The NCCL-free native runtime packaged successfully with ABI `0.1.74`;
the upstream archive verifier passed for CUDA/x86_64, and the manifest records
SM120 support. The unchanged upstream release-host recipe also completed,
including its console build and product archive creation. The complete product
manifest agrees on ABI `0.1.74`; host, runtime manifest and runtime file hashes
were checked. The archive is 803,922,519 bytes compressed and 1,268,733,464 bytes
expanded. The local AMD64 worker image is 2,982,352,801 bytes and reports stock
Mesh `0.78.0`; its dynamic host libraries resolve. Registry publication and GPU
qualification remain pending. The worker retains only fixed NVIDIA driver
search paths in its otherwise isolated environment; it does not inherit the
provider instance secret.
Raw logs, identities, captures and accounting evidence remain private.

## Exact-image local checks

Checked local image ID:
`sha256:3c5db0989a38dd3e634d181d8610c61bfc09f6af997aee5adfc37ce96365f790`.
It is not yet an anonymously available registry reference. Its input product
archive SHA-256 is
`6ec6f1e1c763ee49731fba1910878018c15e5b1ed7295ccf4634c5d8177150b1`.

`deploy/container/qualify_stock_ssh.py` exercises the actual image on a Docker
internal network. Its SSH client is another container on that isolated network;
no public port or provider access is needed. Wrong host pin, unsupported command
and replay were refused. A bounded worker failure erased its join file and left
no worker process. With a valid ephemeral local invite, the harness observed the
worker running, then verified supervisor-signal cancellation and a five-second
lifetime configuration. Both stopped the worker and erased the invite. Test
containers, networks and temporary client keys were removed.

The process check recognizes the executable among arguments, including an AMD64
emulation wrapper on an ARM host. An earlier first-argument-only check missed the
wrapped process; the corrected checks were rerun successfully. These checks are
not proof of GPU execution, admission to the selected live mesh, or destruction
of a billable provider instance. Registry authentication failures created no
rental and consumed none of the separately authorized live budget.

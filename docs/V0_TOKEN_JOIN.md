# Proposed v0: token-based rental attachment

Status: implemented with a published, locally qualified stock worker; remote GPU
and end-to-end acceptance pending.
The certified prototype remains separate from the explicit token-only path.

The v0 goal is to attach one bounded Vast rental to a selected mesh using a
stock Mesh runtime and its join token. The token supplies connection information,
not proof of authorization or worker ownership. Use an explicitly selected mesh
whose admission policy permits the worker; never silently weaken that policy.
The provider can inspect the worker and its inference data. A dedicated mesh is
appropriate where that trust is acceptable.

Implementation and acceptance:

1. Add an explicit token-only launch mode, keeping the existing certified mode
   intact. Deliver the token over the instance's authenticated, host-key-pinned
   SSH connection via stdin; use a private join file, not argv or logs.
2. Provide and qualify an immutable worker image containing stock Mesh and a
   compatible provisioning/supervision path. Preserve bounded lifetime,
   cancellation, independent exact-resource destruction, durable accounting,
   and explicit failure when cleanup or provider absence is uncertain.
3. Bind the operation to the selected mesh and refuse a switch. Observe the
   intended model and a successful request through that mesh before reporting
   token-mode readiness. Never report certificate verification in this mode.
4. Expose the operation through the Mesh plugin and make the desktop consume
   the same versioned interface. The opt-in desktop adapter and plugin now share
   `m8s.operator/v0`; the default historical desktop path still invokes the CLI.
5. Test denied admission, incorrect host identity, token delivery and redaction,
   cancellation and cleanup failure with synthetic fixtures. Qualify the actual
   app, stock Mesh, worker image, inference and confirmed destruction in a fresh
   bounded live run before calling the demo complete.

## Follow-up issue: owner-certified worker admission

Add a bounded, authorized way for an owner node to issue the canonical node
certificate for a specific rented worker, without exposing the owner signing key
to that worker. Bind the request to the selected mesh, worker identity and
limited lifetime; define denial, expiry, replay and revocation behavior. Confirm
whether current upstream Mesh already exposes the necessary host API before
proposing changes there. Integrate and qualify the resulting path against P001,
including certificate verification and owner-routed inference. Token-only v0
must remain clearly identified until this is complete.

## Implementation checkpoint

The branch adds the explicit `token_target` plan field, private token framing,
restricted `join-v0` SSH command, stock worker supervisor, and shared
`m8s.operator/v0` plugin/desktop operation adapter. These are implementation
changes, not a successful live demonstration. The existing certified mode is
preserved and cannot silently fall back to token mode.

Prepare a fresh private plan from [the inert v0 template](v0-plan.template.json)
only after reconciling
existing attempt liabilities. Its zero budgets, unknown accounting status and
unqualified image placeholder deliberately prevent launch. Never substitute the
historical private-fork image. Create the private `control_directory` with mode
0700; leave the fresh attempt `directory` nonexistent. Use canonical absolute paths
and mode 0600 for plan, target and join files. Keep all established budget fields; add
`"token_target": "/absolute/private/target.json"` and omit `attachment`. The target
file is private, operator-selected JSON:

```json
{
  "schema": "m8s.token-target/v0",
  "admission": "token-only-provider-trusted",
  "join_file": "/absolute/private/join-token",
  "api_port": 9337,
  "console_port": 3131,
  "node_id": "replace-with-selected-node-id",
  "mesh_id": "replace-with-selected-mesh-id"
}
```

The join file contains only the selected mesh's invite token. Never put it in
plugin arguments, provider launch metadata, argv, logs, a README or a recording.
Target identity is rechecked before launch and throughout verification. v0 refuses
ambiguous multi-host canary attribution; use a model-free client and one remote
host. The provider may inspect the token and inference data inside its worker.

`Dockerfile.stock` accepts a complete unmodified stock Linux CUDA product and its
SHA-256, with an explicitly pinned runtime base. Use upstream's `just` build and
packaging recipes from a recorded main commit. No private fork artifact or native
metadata patch is a substitute. The official `v0.78.1` release resolved to a
commit outside the checked main tip's ancestry, so it was not accepted as a main
qualification artifact. The matching main product was built and its worker passed local qualification;
[the build contract](../deploy/container/stock-image.json) records its published digest.
Remote GPU acceptance remains a separate gate.

The actual stock-main plugin handshake and a read-only MCP status call passed;
see [the validation checkpoint](V0_VALIDATION.md). Before declaring v0 complete, test denied
admission, wrong SSH identity, cancellation, stale target, quote replay and cleanup
uncertainty; qualify the exact image; perform a newly authorized bounded rental,
real app conversation, and provider-confirmed destruction; then replace the demo
and add a sanitized acceptance summary. The historical GIF remains historical.

Owner certificate issuance follow-up:
https://github.com/Mesh-LLM/m8s-plugin/issues/1

## Current qualification blockers

The checked main host requires Skippy ABI `0.1.74`. The downloaded official
`v0.78.1` Linux CUDA product declares ABI `0.1.66`; its CUDA architecture list
also does not include the SM120 devices required by the current Qwen profile.
It cannot be substituted or relabeled. A matching stock-main Linux CUDA/SM120
product has now been built without altering upstream source or packaging policy.
Its image passed local SSH checks and anonymous digest access. Native GPU execution,
real download behavior and provider-confirmed cleanup remain live acceptance gates.

The Mac's default Rust 1.92 compiler rejects main's `AtomicUsize::try_update`.
The unmodified main host and console built successfully using the installed
Rust 1.95 toolchain (`just mesh`), reporting `0.78.0+g280944`. This does not
establish full-product or remote GPU qualification.

The worker carries forward the prototype's bounded download settings: both Hub
and Xet retry limits are zero, with an isolated cache and implicit Hub credentials
disabled. It also uses one inference slot, a 4096-token context and no draft model.
These controls do not replace the transfer reserve, rental deadline, or independent
cleanup. The actual main binary passed synthetic Xet reconstruction and payload failure
checks. Metadata fallback resolvers can still retry; these settings are not a
global no-retry guarantee. Complete image size and real model transfer accounting
still require qualification before a paid launch plan. See the validation
checkpoint for the exact scope.

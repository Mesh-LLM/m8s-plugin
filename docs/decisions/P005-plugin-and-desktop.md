# P005: m8s plugin and experimental desktop console

Accepted direction, 2026-10-29. Implementation in progress.

## Decision

Publish m8s to Mesh-LLM/m8s-plugin. Preserve the provider-neutral Rust core,
accounting journals, certificate admission and independent cleanup. A Mesh plugin
is an adapter to the existing daemon, not another scheduler or budget ledger.
Keep inference on certified remote mesh nodes; a local inference gateway is a
future explicit backend mode, not an implicit change to rental identity.

The experimental desktop fork is joabzzz/m8s-app, based on Mesh-LLM/desktop-app.
Retain its React/Tauri visual language and chat. Add fleet, providers and inference
navigation with measured activity tiles. Desktop release distribution is out of
scope; use the app for testing and demo recordings. Credentials remain backend-only.

The user explicitly authorized contribution to the organization repository and
use of /Users/joabz/Development/m8s/.env for Vast testing, up to available loaded
credit, superseding historical session dollar caps. Do not top up the account.
Each actual attempt still records a finite lease, current available funding and
independently surviving exact-resource cleanup. Preserve prior liabilities.

## Integration issues and acceptance

- Existing desktop proof creates a dedicated mesh. Attaching rentals to an
  existing embedded desktop identity requires a supported owner signing bridge.
- Reconcile current Mesh SDK/plugin versions with the proven MP-001 native fork.
  Never export the owner private key to the WebView or a rental.
- Offer browsing needs a typed read-only API and fresh quote validation at launch.
- Metrics must distinguish peer-advertised capacity, measured inference speed,
  GPU utilization and unavailable observations. Missing data is never zero.
- Provider support: Vast has live evidence; Nebius is unimplemented.
- Rental teardown must survive desktop/plugin exit. Stopped is not destroyed.
- Public source publication requires scanning tracked source and exported history
  for credentials and machine-specific private artifacts before upload.
- Plugin readiness requires a real host handshake and calls against m8sd;
  compilation alone is insufficient. UI mocks do not establish GPU acceptance.

## Owner attachment clarification

Passive client and inference host are both valid local controller roles. Paid
operations require an authenticated attachment to the selected local node and
owner plus explicit controller authorization. Node role alone never grants or
removes rental authority. Verify mesh policy and persist owner/node/mesh binding;
refuse silent fleet reassignment when the desktop changes mesh or instance.

## Why keep the inference host on rented compute?

A physical device and a Mesh node are different concepts. Multiple instances
can coexist on a device when identities, ports and state directories are distinct.
That does not automatically create separate security boundaries: processes under
the same OS user still share much of the same trust domain.

A gateway on the laptop could advertise an inference service backed by a remote
runtime, but that is a separate backend architecture. It adds a laptop dependency
and another authenticated transport, and requires accurate remote capacity,
routing, failure and model reporting. It does not eliminate the need to authorize
the rented worker or protect its model endpoint. It is not inherently more secure.

For this integration, the rented container remains the certified inference host.
The laptop's client or host role is independent of its authority to manage that
rental. The certificate authorizes the remote node; it is not evidence that the
owner has a local GPU. The dedicated demo owner keeps its signing material local.
A supported owner-signing API is still needed to attach arbitrary existing nodes
without exporting their permanent key (see P004).

The desktop now ports the prior bounded rental adapter. Its fleet displays the
verified dedicated owner, clears that display on failed observations, and keeps
navigation within that session. It still requires the dedicated passive owner;
general host/client attachment remains an explicit acceptance gap.

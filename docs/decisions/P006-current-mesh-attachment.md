# P006 — App-owned current-mesh attachment

The prototype launch accepts an optional private `attachment` descriptor. Without
it, the existing dedicated owner-client path is unchanged. With it, the controller
uses the desktop's existing passive or hosting node for its invite and inference
route; it does not start or stop a separate owner client.

The descriptor contains a loopback port, an expiring capability, and the selected
node/mesh/certificate identity. The app custodies the owner key and performs
canonical exact-time signing once per admitted grant. The controller's nonce
consumption, certificate verifier, iroh transport, budget ledger and independent
provider cleanup remain in place. The target is checked on attachment, immediately
before CREATE, before provisioning and on signature issuance. Failure refuses or
cleans up the attempt; it never falls back to a different mesh.

`prototype vast-offers` projects a bounded, read-only catalog across memory
classes. `prototype vast-quote` freshly qualifies the exact selected offer for the
pinned Qwen 27B profile. Catalog estimates are not claims of runtime compatibility.

This is an experimental desktop bridge, not generic plugin authorization. The
private capability grants one bounded issuance to the trusted local controller.
It is never delivered to a rented host or to WebView JavaScript. The app's one
attempt per plan prevents accidental CREATE retries; multi-attempt scheduling
and node-specific inference routing need separate qualification.

Exact-offer review carries a machine ID search hint and verifies the returned
machine and offer identities. This avoids losing the selected offer in a bounded
whole-market sample. It never substitutes another offer. Native preparation
progress remains coarse; container-running is not model readiness.

The desktop SDK and remote runtime can expose different exact public model IDs.
Discovery permits the pinned HF alias or its pinned native source identity, then
sends the observed ID for the single canary request. Response validation still
requires the pinned native source identity. See the sanitized
[current-mesh receipt](../HISTORICAL_EVIDENCE.md) for
live attachment/removal evidence and the canary qualification status.

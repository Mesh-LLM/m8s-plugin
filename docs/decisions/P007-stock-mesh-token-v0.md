# P007: Explicit token-only admission for stock-Mesh v0

Status: implemented experimentally; stock runtime built; live inference acceptance pending.

P001 remains the desired owner-certified admission contract. The scoped review
of upstream main found no documented plugin host operation for issuing the
required worker certificate. [Issue #1](https://github.com/Mesh-LLM/m8s-plugin/issues/1)
tracks that capability; no owner signing key is exported to a rental.

For v0, an operator may explicitly choose `token-only-provider-trusted` admission
and a private target file. The join token is delivered through restricted SSH
with the exact provider-observed instance host key pinned. Stock Mesh reads a
private `--join-file`. This protects token delivery against an unrecognized SSH
endpoint, but does not protect it from the provider and does not certify worker
ownership. The worker's existing Mesh admission policy is not bypassed.

The selected local node must be an isolated passive client. Readiness requires
the exact selected mesh, the sole worker identity acknowledged over SSH, model
discovery, and a successful canary through the selected local API. Live status
also requires that worker to remain present. Every token-mode result reports
`certificate_verified: false`; certified mode cannot fall back to this mode.

The plugin and opt-in desktop adapter share `m8s.operator/v0`. Requests cannot
replace credentials, target files, image, executable or budget policy. Quotes
expire, launch intent is durable before effects, and consumed attempts cannot
be retried through the interface. Existing independent cleanup, exact absence
confirmation and conservative accounting remain required. The owner watchdog
survives launcher exit, not loss of the owner machine or provider outages. The
remote supervisor's deadline stops inference but does not destroy the billable
instance. Uncertain cleanup retains liability and blocks another attempt.

This is a constrained demonstration path, not general multi-host attribution or
persistent instances. A build against the main SDK is not live qualification.
The exact stock worker product, plugin host integration, denied admission,
cleanup and real desktop conversation must be verified before recording a new
demo. Until then the README shows an explicitly labeled architecture preview.

See [the implementation and acceptance gates](../V0_TOKEN_JOIN.md).

Read-only `status` and `offers` do not take the exclusive mutation lock. Quotes,
launch and removal remain serialized, and durable launch intent still precedes
effects. Status may observe an incomplete new file and must project it
conservatively as unready. A UI status poll must not make concurrent provider
discovery report missing credentials or prevent cleanup/status observation.


### Startup observations after the first stock-image attempt

The first paid stock-image attempt did not discover a worker or complete a canary
within its readiness deadline. The UI removed the rental and an independent
provider inventory confirmed absence. Its reservation remains retained because
billing is unsettled. Provider running state alone did not identify whether
startup was waiting on model transfer, runtime initialization or mesh admission.

The restricted SSH key therefore also permits the exact read-only `status-v0`
command. It returns a bounded, whitelisted projection of worker phase, process,
cache and network counters; it cannot execute a shell or expose raw logs, paths,
keys or invites. Status does not consume the one-shot join claim. The CLI polls
through the same pinned host key and records observations privately. The plugin
only exposes observations bound to the exact resource/image and at most 30
seconds old. A worker reporting its mesh identity is not proof of peer discovery
or inference. Existing canary gates and cleanup deadlines remain authoritative.


Worker observations also project bounded requested/hosted/runtime model counts,
known loading/serving flags, and a count of reported runtime errors. Lists are
capped at 256 entries and a partial-snapshot flag marks truncation. Error text,
model paths and arbitrary lifecycle strings are never returned. Missing fields
remain unknown. Neither these counters nor a worker's serving flag bypass the
selected-mesh discovery and owner-routed canary gates.

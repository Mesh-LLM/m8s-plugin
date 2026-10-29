# P001: Canonical native identity and secure admission

Status: implemented for the opt-in prototype; demonstrated 2026-10-23.

## Context

A provider container must run the actual mesh node with a verifiable owner
certificate. Provider metadata, an SSH connection, a listening port, and a
successful process spawn do not individually establish admission or readiness.

## Decision

The owner retains the signing key. The worker receives only its node material
and public certificate. Provisioning uses restricted SSH with the host key
obtained through the exact instance's authenticated provider log operation.
Secret frame fields travel through stdin, never provider launch metadata.
The fixed provisioning acknowledgments identify progress, not admission.

The owner initiates pinned direct iroh TLS to the provider-observed UDP mapping.
The durable one-use nonce and canonical native identity verifier bind admission
to the intended resource. Required-certificate client mode preserves the
certified native key; substituting an ephemeral client identity is invalid.
Missing mappings or failed TLS stop the attempt and retain cleanup obligations.
Control heartbeats stay within the receiver's idle timeout.

## Consequences

The prototype requires usable public TCP provisioning and UDP transport paths.
Advertised mappings alone cannot prove that a host network permits the path.
A failed connection never permits plaintext fallback or silent rerental.
Production relay policy, NAT diversity, and recovery need separate qualification.

## Evidence and implementation

- [Live receipt](../HISTORICAL_EVIDENCE.md)
- [Native source revisions](../../deploy/container/native-source.json)
- [Bootstrap contract](../../crates/m8s-bootstrap/README.md)
- [Provisioning protocol](../../crates/m8s-provider/src/vast/provision_protocol.rs)

## Stock-Mesh v0 exception

[P007](P007-stock-mesh-token-v0.md) records the explicitly selected, weaker
join-token admission path. It does not satisfy this ADR's owner certificate
requirement. Certificate issuance is tracked in
[issue #1](https://github.com/Mesh-LLM/m8s-plugin/issues/1); token-mode results must
never claim certified admission.

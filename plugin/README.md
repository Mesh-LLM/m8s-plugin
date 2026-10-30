# m8s Mesh plugin

Experimental `compute` tool for the versioned `m8s.operator/v0` interface, plus
legacy read-only controller `status`. Stock-Mesh live acceptance is pending.

`compute` accepts exactly one operation:

```json
{"operation":"offers"}
{"operation":"quote","offer_id":123,"machine_id":456}
{"operation":"launch","quote_id":"value returned by a fresh quote"}
{"operation":"status"}
{"operation":"remove"}
```

The plugin inherits `M8S_CLI` and `M8S_V0_PLAN` from its host. These must select an
owned executable and a private, explicit token-only launch plan; no request can
change credentials, target files, image or financial policy. One durable attempt
per control directory; a consumed intent is never reset by a tool call. Quotes
expire after 30 seconds. Quotes include a conservative full reserve estimate
and reject prices that exceed the selected plan, including transfer, cleanup
and contingency. Launch independently rechecks provider conditions and
budget before CREATE. Do not treat an offer/quote as a rental or readiness proof.

The standalone `m8s-plugin --request` mode reads the same operation JSON on stdin
and returns the same versioned response. The desktop uses this local adapter;
Mesh uses plugin IPC/MCP. They share the operation implementation rather than
separate rental logic. Raw controller errors and private files are never returned.

Build: `cargo build --manifest-path plugin/Cargo.toml --locked -j2`.
Package: `sh plugin/package.sh`, then install with stock Mesh:

```sh
mesh-llm plugins install --archive plugin/target/m8s-0.1.0.tar.gz --name m8s --version 0.1.0
```

Enable the installed plugin in the selected stock Mesh config:

```toml
[[plugin]]
name = "m8s"
enabled = true
```

Export `M8S_CLI` and `M8S_V0_PLAN` before starting that host. The plan can be
prepared from [the inert token-only template](../docs/v0-plan.template.json);
replace its placeholders only after reconciling the operator's budget and
selecting a qualified stock worker image. Keep the plan and join token private.
Start or select a model-free private client, bind the target file to that exact
node and mesh, then request offers and a fresh quote. `launch` consumes that quote
only once. Poll `status` for actual canary verification and use `remove` to
request cleanup; wait for provider-confirmed removal. Never retry a failed
launch by deleting its intent or accounting files. See the
[v0 runbook](../docs/V0_TOKEN_JOIN.md) for admission and cleanup limits.

The controller executable is configured separately. For legacy status only,
set `M8S_CONTROL_SOCKET` to the owner controller socket.

The SDK pin is upstream main `2809443345ea8134db62a8b433f5bb2bb5f1d123`.
Actual stock-main host registration and an MCP status call passed; see
[the validation checkpoint](../docs/V0_VALIDATION.md). Paid end-to-end acceptance
remains pending.

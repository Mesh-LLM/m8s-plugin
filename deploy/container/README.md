# Worker container recipes

Stock-Mesh token-only v0 is experimental. A successful stock-main GPU rental and
replacement demo are still pending; see the [validation checkpoint](../../docs/V0_VALIDATION.md).
Historical build/run receipts and accounting notes were preserved privately and
removed from this source guide. [Historical evidence scope](../../docs/HISTORICAL_EVIDENCE.md)
explains the earlier certified private-fork demonstrations. They are not v0 proof.

## Stock-Mesh worker

`Dockerfile.stock` packages a complete unmodified upstream Linux CUDA product,
`token_provision.py`, and the shared restricted SSH setup in `vast_provision.py`.
It contains no model weights or owner signing key. Mesh downloads the pinned
Qwen artifact after receiving the join token over host-key-verified SSH stdin.

Build the full native runtime, host, console and product archive using upstream
`just` recipes from an exact main commit. The Linux builder needs the upstream
Rust toolchain, CUDA toolkit, C/C++ compiler, CMake, Ninja, just, sccache,
Node/pnpm, Python, and **patchelf**. Limit compilation to two jobs. Preserve the
builder filesystem across failures: native compilation can finish before a
packaging dependency failure. Do not edit ABI or architecture metadata to force
acceptance, or substitute a private-fork runtime.

Prepare a minimal context containing only:

- `Dockerfile.stock`, `vast_provision.py`, and `token_provision.py`;
- the complete product archive copied as `stock-product.tar.gz`.

Supply `RUNTIME_BASE` as an explicitly selected immutable image digest with
compatible native dependencies, and `STOCK_PRODUCT_SHA256` as the archive hash.
The archive must contain its host executable, native runtime manifest/libraries,
console assets and required licenses. No checkout, `.git`, credentials, caches,
private keys, run receipts or weights belong in the context.

```sh
docker buildx build --platform linux/amd64 -f Dockerfile.stock \
  --build-arg RUNTIME_BASE="$PINNED_RUNTIME_DIGEST" \
  --build-arg STOCK_PRODUCT_SHA256="$PRODUCT_SHA256" \
  --load -t m8s-stock-worker:qualification "$BUILD_CONTEXT"
```

Before renting, check the product source/version, ABI, SM120 support, dynamic
libraries, final image size, private provisioning, host-key rejection, token
redaction, cancellation and bounded shutdown. The image must fit the launch
plan's disk and transfer reserves. Pin the published worker by digest. A worker
acknowledgement is not admission or inference proof. The live acceptance gates
and independent exact-resource cleanup remain in the [v0 runbook](../../docs/V0_TOKEN_JOIN.md).

The worker limits inference to one slot, a 4096-token context and no draft model.
It requests zero Hub/Xet retries. Reproduce the synthetic Xet failure checks
against an already built stock binary with:

```sh
node deploy/container/qualify_stock_download.mjs /absolute/mesh-llm cas
node deploy/container/qualify_stock_download.mjs /absolute/mesh-llm payload
```

These test failed reconstruction/payload requests using local synthetic services,
not a real model transfer. Metadata fallback resolvers can still retry; keep the
transfer reserve and independent cleanup. The supervisor stops the worker at its
deadline; stopping a process does not destroy the billable provider instance.

## Retained certified prototype recipes

These remain for the legacy certified mode, not as stock-main qualification:

| Recipe | Purpose |
| --- | --- |
| `Dockerfile.source` | Full native CPU/CUDA product from hash-pinned source archives and prepared offline bases. |
| `Dockerfile.smoke` | Online dependency preparation followed by offline CPU product compilation. |
| `Dockerfile.flint` | CUDA runtime and agent without weights; Mesh downloads the pinned model. |
| `Dockerfile.agent`, `Dockerfile.agent-update` | Refresh the agent over an immutable existing runtime image, retaining its model/provisioning contract. |
| `Dockerfile`, `Dockerfile.vast` | Earlier packaged-product/provisioning recipes. |

Source recipes take clean `git archive` inputs and their SHA256 values. Their
base images must contain all required tools/dependencies; missing inputs fail
rather than fetching or substituting during offline compilation. The model
contracts are public artifact metadata, not run receipts. Keep `check-model`
limits scoped to the selected recipe: the small CPU fixture's 2 GiB bound does
not apply to the separate pinned Qwen 27B download profile.

The certified path retains its required certificate, actual-container binding,
nonce burn, canonical TLS, native node-key binding and exact-resource teardown.
Do not remove these gates to make a legacy recipe run. Images use Tini rather
than running the agent or SSH supervisor directly as PID 1.

[RELEASE.md](RELEASE.md) documents the legacy release tooling: source archives,
layer keys, checksums, unsigned provenance, digest comparisons and context audit.
Its existing private-fork pins do not describe a stock-main v0 image.

## Source checks

```sh
python3 -B -m unittest discover -s deploy/container/tests -v
```

These use synthetic files, archives and fake programs; they make no provider
calls and do not establish real GPU inference or provider cleanup.

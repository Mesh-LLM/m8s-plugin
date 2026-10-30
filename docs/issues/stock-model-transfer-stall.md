# Stock-main model transfer stalls after partial cache growth

Status: bounded recovery verified in a live stock-GPU run; upstream transfer
failure cause remains open. Final inventory check and reviewed showcase publication completed; see
[V0_ACCEPTANCE.md](../V0_ACCEPTANCE.md).

With stock main 280944334 on both the app-owned passive client and remote worker,
token-only attachment succeeded. The owner observed the intended remote peer and
GPU. The pinned Qwen 3.8 27B transfer reached 5,579,017,927 cache bytes and then
showed no growth for approximately eight minutes, retaining an incomplete file.
The worker API stayed reachable; there were no hosted models or completed canary.
The attempt was removed with independent provider absence confirmation.

The previous attempt downloaded approximately 16.5 GB but could not attach
because the older app runtime's owner-control endpoint evicted its main relay
connection. Stock main fixes that separate issue. Do not equate the two failures
or infer that the transfer stall is necessarily a relay problem.

Existing observations omit raw worker logs and private material. They distinguish
process health, mesh identity and cache/network counters, but not the underlying
model lifecycle error. Before another paid attempt:

- A bounded projection of model counts, loading/serving flags and runtime-error
  counts is implemented for the next qualified worker. Raw error text, paths
  and tokens are excluded. Local checks do not establish the live stall cause.
- Reproduce connected-but-stalled transfer behavior locally; distinguish request
  retry limits from a progress deadline. A synthetic stall does not establish
  the live failure's cause.
- Qualify any worker change against the exact image and retain existing startup,
  canary and cleanup bounds. Do not reset consumed claims or unsettled liabilities.
- Use the standing funded-credit authorization, with fresh balance/prices and
  reconciliation of actual outstanding liabilities; the earlier $6 session cap
  is superseded. No successful media may replace the preview until actual
  owner-routed inference, app conversation and provider absence all pass.


Local reproduction: run
`node deploy/container/qualify_stock_download.mjs /absolute/stock-mesh stall`.
The synthetic payload endpoint sends headers and one byte, then leaves the body
unfinished. Against stock 0.78.0+g280944, one payload request remained pending
until the harness killed the downloader at 20 seconds. No request retry occurred.
This demonstrates why retry limits are not progress deadlines; it does not prove
that the live stalled transfer had the same cause. The existing worker/owner
lifetime bounds remain essential.


The next qualified-image attempt also joined the selected mesh, then stopped
cache growth at approximately 7.10 GB with one reported runtime error and zero
hosted models. It was removed early; an independent full provider inventory
confirmed no instances or volumes remained. This establishes a runtime failure,
not merely slow progress, but its error count alone does not identify the cause.
No inference or replacement demo was accepted.

The worker now projects a fixed set of boolean keyword hints from bounded runtime
errors (download, timeout, connection, authentication, rate limit, disk, memory,
native runtime). These are heuristic observations, not diagnoses or readiness
signals. Every transport layer accepts only those named boolean fields; raw error
text, URLs, tokens and paths remain excluded. The changed image requires fresh
qualification before use.


A subsequent isolated macOS stock-main download reproduced an Xet response-body
decode failure at about 7.76 GB. A second CLI invocation using the same cache
completed the 16,464,440,224-byte file. That invocation enabled one Hub/Xet retry,
so it does not isolate the effect of retry flags from process restart/resume.
Stock source explicitly resumes partial Xet files on a new invocation. This
local result is not proof that every remote failure has the same cause.

The supervisor now permits exactly one process restart for a reported download
failure, with one requested model and no hosted/loading/serving model. It rejects
known auth, disk, memory and native-runtime error hints. It reaps the old process
group, retains the explicit node-key path and cache, verifies unchanged identity,
and preserves the original deadline and single provisioning claim. Hub/Xet
request retries remain zero. The 64 GB transfer reserve already covers two full
model paths and the image. Remote inference still requires live verification.


Live recovery result: the next rental used the single restart, completed the
model download, served the owner-routed canary and both app chat turns, and was
removed with controller and watchdog absence receipts. The separate final account
inventory check subsequently confirmed complete empty inventories. This verifies the mitigation for this run; it does
not establish the underlying cause of every Xet response-body decode failure.

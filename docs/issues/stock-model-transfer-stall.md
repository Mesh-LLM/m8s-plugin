# Stock-main model transfer stalls after partial cache growth

Status: open; no successful stock-GPU inference or replacement demo.

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

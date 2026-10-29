// Qualification harness for an already built, unmodified stock Mesh executable.
// Uses an isolated HOME/cache and local synthetic Hub/CAS server. Retains private
// diagnostics in a fresh temporary directory. Does not consume provider credentials.
import http from "node:http";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
const [binary, scenario = "payload"] = process.argv.slice(2);
if (
  !binary ||
  !path.isAbsolute(binary) ||
  !["cas", "payload"].includes(scenario)
) {
  throw Error(
    "Usage: node qualify_stock_download.mjs /absolute/mesh-llm [cas|payload]",
  );
}
const root = await fs.mkdtemp(path.join(os.tmpdir(), "m8s-stock-download-"));
await fs.chmod(root, 0o700);
const requests = [];
const commit = "a".repeat(40),
  hash = "b".repeat(64);
const server = http.createServer((req, res) => {
  requests.push({
    method: req.method,
    path: req.url,
    range: req.headers.range ?? null,
  });
  const base = `http://127.0.0.1:${server.address().port}`;
  if (req.url.includes("/api/models/") && req.url.includes("/revision/")) {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(
      JSON.stringify({
        id: "m8s-synthetic/unavailable",
        sha: commit,
        siblings: [
          {
            rfilename: "test.gguf",
            size: 1024,
            lfs: { size: 1024, sha256: hash, pointerSize: 100 },
          },
        ],
      }),
    );
  } else if (req.url.includes("/xet-read-token/")) {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(
      JSON.stringify({
        accessToken: "synthetic",
        exp: Math.floor(Date.now() / 1000) + 3600,
        casUrl: base + "/cas",
      }),
    );
  } else if (req.method === "HEAD" && req.url.endsWith("/test.gguf")) {
    res.writeHead(200, {
      ETag: hash,
      "X-Repo-Commit": commit,
      "X-Xet-Hash": hash,
      "X-Linked-Size": "1024",
      "Content-Length": "1024",
    });
    res.end();
  } else if (req.url.startsWith("/cas/v2/reconstructions/")) {
    if (scenario === "cas") {
      res.writeHead(503, { "Content-Type": "application/json" });
      res.end('{"error":"synthetic unavailable"}');
      return;
    }
    if (req.headers.range?.startsWith("bytes=0-")) {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          offset_into_first_range: 0,
          terms: [{ hash, unpacked_length: 1024, range: { start: 0, end: 1 } }],
          xorbs: {
            [hash]: [
              {
                url: base + "/payload",
                ranges: [
                  {
                    chunks: { start: 0, end: 1 },
                    bytes: { start: 0, end: 1023 },
                  },
                ],
              },
            ],
          },
        }),
      );
    } else {
      res.writeHead(416);
      res.end();
    }
  } else {
    res.writeHead(req.url === "/payload" ? 503 : 404, {
      "Content-Type": "application/json",
    });
    res.end('{"error":"synthetic unavailable"}');
  }
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const log = await fs.open(root + "/output.log", "wx", 0o600);
const env = {
  PATH: process.env.PATH,
  HOME: root,
  HF_HOME: root + "/hf",
  HF_HUB_CACHE: root + "/hf/cache",
  HF_ENDPOINT: `http://127.0.0.1:${server.address().port}`,
  HF_HUB_DISABLE_IMPLICIT_TOKEN: "true",
  MESH_HF_RETRY_MAX_ATTEMPTS: "0",
  HF_XET_CLIENT_RETRY_MAX_ATTEMPTS: "0",
  MESH_LLM_ANALYTICS: "0",
  DO_NOT_TRACK: "1",
};
const child = spawn(
  binary,
  ["download", "m8s-synthetic/unavailable@" + commit + "/test.gguf"],
  { env, stdio: ["ignore", log.fd, log.fd] },
);
let timedOut = false;
const timer = setTimeout(() => {
  timedOut = true;
  child.kill("SIGKILL");
}, 20000);
const code = await new Promise((r) => child.on("exit", r));
clearTimeout(timer);
server.close();
await log.close();
const selected = requests.filter((r) =>
  scenario === "cas" ? r.path.startsWith("/cas/") : r.path === "/payload",
);
const selectedCounts = {};
for (const r of selected) {
  const key = r.method + " " + r.path + " " + r.range;
  selectedCounts[key] = (selectedCounts[key] ?? 0) + 1;
}
const result = {
  scenario,
  code,
  timedOut,
  passed:
    code !== null &&
    code !== 0 &&
    !timedOut &&
    selected.length > 0 &&
    Object.values(selectedCounts).every((n) => n === 1),
  selected_request_counts: selectedCounts,
  scope:
    "Synthetic HTTP 503 failure; no real weights, provider calls or GPU inference. Only the selected Xet failure requests are counted; metadata fallback behavior is outside this assertion.",
};
await fs.writeFile(root + "/result.json", JSON.stringify(result, null, 2), {
  mode: 0o600,
});
console.log(JSON.stringify({ ...result, evidence_directory: root }, null, 2));
process.exitCode = result.passed ? 0 : 1;

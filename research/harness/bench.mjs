#!/usr/bin/env node
// Benchmark a docx-to-pdf-wasm worker.
//
// Usage:
//   node bench.mjs <worker-base-url> <report-json-path> [--quick] [--pid=N]
//
// Example:
//   node bench.mjs http://127.0.0.1:8787 /tmp/js-worker-bench.json
//
// Phases:
//   1. warmup          — 5 sequential PDF requests on a small doc (results discarded)
//   2. corpus sweep    — all 50 corpus docs × 3 formats (PDF / HTML / MD) sequential
//   3. hot sequential  — same medium doc, 50 sequential PDF requests
//   4. hot c=10        — same doc, 50 PDF requests with max-in-flight=10
//   5. hot c=25        — same doc, 50 PDF requests with max-in-flight=25
//   6. mixed c=10      — 50 mixed (PDF/HTML/MD) at c=10
//
// Memory: samples RSS of the workerd process every 200 ms throughout the run,
// reports min/mean/max per phase.

import { readFile, writeFile } from "node:fs/promises";
import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { spawn, execSync } from "node:child_process";

const argv = process.argv.slice(2);
const baseUrl = argv[0];
const reportPath = argv[1];
const quick = argv.includes("--quick");
const pidArg = argv.find((a) => a.startsWith("--pid="));
const explicitPid = pidArg ? parseInt(pidArg.split("=")[1], 10) : null;

if (!baseUrl || !reportPath) {
  console.error("usage: bench.mjs <worker-base-url> <report-json-path> [--quick] [--pid=N]");
  process.exit(2);
}

const ROOT = "/Users/fforres/GITHUB/skyward/wasm-docx-to-pdf/research/fixtures";
const HOT_DOC = `${ROOT}/complex/cdc_ngs_validation.docx`;
const WARMUP_DOC = `${ROOT}/tier1/test.docx`;

const TIERS = ["tier1", "tier2", "tier3", "complex"];
const FORMATS = {
  pdf: { path: "/convert" },
  html: { path: "/convert/html" },
  markdown: { path: "/convert/markdown" },
};

function listCorpus() {
  const docs = [];
  for (const tier of TIERS) {
    const dir = join(ROOT, tier);
    for (const name of readdirSync(dir)) {
      if (!name.endsWith(".docx")) continue;
      const full = join(dir, name);
      const size = statSync(full).size;
      docs.push({ path: full, name, tier, size });
    }
  }
  return docs;
}

// ----------------------------------------------------------------- memory

function findWorkerdPids() {
  try {
    const out = execSync("pgrep -f workerd").toString().trim();
    return out.split("\n").map((s) => parseInt(s.trim(), 10)).filter((n) => !isNaN(n));
  } catch {
    return [];
  }
}

function startMemoryPoller(pids, intervalMs = 200) {
  if (pids.length === 0) {
    return { samples: [], stop: () => {}, pids };
  }
  // Sum RSS across all workerd processes. Some wrangler configs spawn a
  // supervisor + a worker pool; we want total footprint.
  const list = pids.join(",");
  const samples = []; // {t_ms, rss_kb}
  // Spawn a small sh poller. We don't use --no-headers (Linux-only); on macOS
  // we strip headers manually.
  const cmd = `
    while true; do
      total=0
      for p in ${pids.join(" ")}; do
        v=$(ps -o rss= -p $p 2>/dev/null | tr -d ' ')
        if [ -n "$v" ]; then total=$((total + v)); fi
      done
      printf "%d\\n" "$total"
      sleep ${intervalMs / 1000}
    done
  `;
  const child = spawn("sh", ["-c", cmd]);
  let buf = "";
  child.stdout.on("data", (chunk) => {
    buf += chunk.toString();
    const lines = buf.split("\n");
    buf = lines.pop();
    for (const line of lines) {
      const rss = parseInt(line.trim(), 10);
      if (!isNaN(rss) && rss > 0) {
        samples.push({ t_ms: performance.now(), rss_kb: rss });
      }
    }
  });
  child.stderr.on("data", () => {}); // swallow
  return {
    samples,
    pids,
    stop: () => {
      try {
        child.kill("SIGKILL");
      } catch {}
    },
  };
}

function memStatsFromSamples(samples) {
  if (samples.length === 0) return null;
  const sorted = samples.map((s) => s.rss_kb).sort((a, b) => a - b);
  const sum = sorted.reduce((a, b) => a + b, 0);
  const toMb = (kb) => +(kb / 1024).toFixed(1);
  return {
    samples: samples.length,
    min_mb: toMb(sorted[0]),
    p50_mb: toMb(sorted[Math.floor(sorted.length * 0.5)]),
    p95_mb: toMb(sorted[Math.floor(sorted.length * 0.95)]),
    max_mb: toMb(sorted[sorted.length - 1]),
    mean_mb: toMb(sum / sorted.length),
  };
}

// ----------------------------------------------------------------- stats

function percentile(sorted, p) {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length));
  return sorted[idx];
}

function statsOf(samples) {
  if (samples.length === 0) {
    return { count: 0 };
  }
  const sorted = [...samples].sort((a, b) => a - b);
  const sum = samples.reduce((a, b) => a + b, 0);
  return {
    count: samples.length,
    min_ms: +sorted[0].toFixed(2),
    p50_ms: +percentile(sorted, 50).toFixed(2),
    p95_ms: +percentile(sorted, 95).toFixed(2),
    p99_ms: +percentile(sorted, 99).toFixed(2),
    max_ms: +sorted[sorted.length - 1].toFixed(2),
    mean_ms: +(sum / samples.length).toFixed(2),
  };
}

// ----------------------------------------------------------------- request

async function postConvert(format, docxBytes) {
  const { path } = FORMATS[format];
  const t0 = performance.now();
  let status = 0;
  let outLen = 0;
  let errMsg = "";
  try {
    const r = await fetch(`${baseUrl}${path}`, {
      method: "POST",
      headers: { "content-type": "application/octet-stream" },
      body: docxBytes,
    });
    status = r.status;
    const body = await r.arrayBuffer();
    outLen = body.byteLength;
    if (status !== 200) {
      errMsg = new TextDecoder().decode(body).slice(0, 200);
    }
  } catch (e) {
    errMsg = String(e).slice(0, 200);
  }
  const elapsed = performance.now() - t0;
  return { status, elapsed_ms: elapsed, out_bytes: outLen, error: errMsg };
}

async function withLimit(items, limit, fn) {
  const results = new Array(items.length);
  let next = 0;
  async function worker() {
    while (true) {
      const i = next++;
      if (i >= items.length) return;
      results[i] = await fn(items[i], i);
    }
  }
  const ws = Array.from({ length: Math.min(limit, items.length) }, worker);
  await Promise.all(ws);
  return results;
}

// ----------------------------------------------------------------- phases

async function phase(name, poller, runner) {
  process.stdout.write(`▶ ${name} ... `);
  const memStartIdx = poller.samples.length;
  const t0 = performance.now();
  const samples = [];
  const errors = [];
  const out = await runner((s) => samples.push(s), (e) => errors.push(e));
  const wallclock = performance.now() - t0;
  // Brief sleep to let the poller capture trailing memory growth
  await new Promise((r) => setTimeout(r, 300));
  const memSlice = poller.samples.slice(memStartIdx);
  const mem = memStatsFromSamples(memSlice);
  const stat = statsOf(samples);
  console.log(
    `done in ${(wallclock / 1000).toFixed(2)}s ` +
      `(n=${stat.count} p50=${stat.p50_ms ?? "-"}ms p95=${stat.p95_ms ?? "-"}ms ` +
      `errors=${errors.length} rps=${(stat.count / (wallclock / 1000)).toFixed(1)} ` +
      `rss_peak=${mem?.max_mb ?? "-"}MB)`,
  );
  return {
    name,
    wallclock_ms: +wallclock.toFixed(2),
    throughput_rps: +(stat.count / (wallclock / 1000)).toFixed(2),
    errors_count: errors.length,
    errors_sample: errors.slice(0, 5),
    memory: mem,
    ...stat,
    extra: out ?? null,
  };
}

async function main() {
  const corpus = listCorpus();
  const pids = explicitPid ? [explicitPid] : findWorkerdPids();
  console.log(`bench target: ${baseUrl}`);
  console.log(`corpus: ${corpus.length} docs across ${TIERS.length} tiers`);
  console.log(`quick mode: ${quick}`);
  console.log(`memory pids: ${pids.length > 0 ? pids.join(",") : "(none found — memory stats disabled)"}`);
  console.log();

  const poller = startMemoryPoller(pids);

  const warmupBytes = await readFile(WARMUP_DOC);
  const hotBytes = await readFile(HOT_DOC);

  // 1. Warmup
  const warmup = await phase(
    "phase 1: warmup (5x small doc, discarded)",
    poller,
    async (_push, pushErr) => {
      for (let i = 0; i < 5; i++) {
        const r = await postConvert("pdf", warmupBytes);
        if (r.status !== 200) pushErr(`warmup #${i}: ${r.status} ${r.error}`);
      }
      return null;
    },
  );

  // 2. Corpus sweep
  const formatsToSweep = quick ? ["pdf"] : ["pdf", "html", "markdown"];
  const corpusBytes = await Promise.all(
    corpus.map(async (d) => ({ ...d, bytes: await readFile(d.path) })),
  );
  const corpusDetail = [];
  const corpus_sweep = await phase(
    `phase 2: corpus sweep (${corpus.length} docs × ${formatsToSweep.length} formats sequential)`,
    poller,
    async (push, pushErr) => {
      for (const fmt of formatsToSweep) {
        for (const d of corpusBytes) {
          const r = await postConvert(fmt, d.bytes);
          push(r.elapsed_ms);
          if (r.status !== 200) {
            pushErr(`${d.tier}/${d.name} (${fmt}): ${r.status} ${r.error}`);
          }
          corpusDetail.push({
            doc: `${d.tier}/${d.name}`,
            in_bytes: d.size,
            format: fmt,
            status: r.status,
            elapsed_ms: +r.elapsed_ms.toFixed(2),
            out_bytes: r.out_bytes,
          });
        }
      }
      return null;
    },
  );

  // 3. Hot sequential
  const hot_sequential = await phase(
    "phase 3: hot path (50x medium doc, sequential, PDF)",
    poller,
    async (push, pushErr) => {
      for (let i = 0; i < 50; i++) {
        const r = await postConvert("pdf", hotBytes);
        push(r.elapsed_ms);
        if (r.status !== 200) pushErr(`hot-seq #${i}: ${r.status} ${r.error}`);
      }
      return null;
    },
  );

  // 4. c=10
  const hot_concurrent_10 = await phase(
    "phase 4: concurrent 50 reqs c=10 (PDF, same doc)",
    poller,
    async (push, pushErr) => {
      const tasks = Array.from({ length: 50 }, (_, i) => i);
      await withLimit(tasks, 10, async (i) => {
        const r = await postConvert("pdf", hotBytes);
        push(r.elapsed_ms);
        if (r.status !== 200) pushErr(`c10 #${i}: ${r.status} ${r.error}`);
      });
      return null;
    },
  );

  // 5. c=25
  const hot_concurrent_25 = await phase(
    "phase 5: concurrent 50 reqs c=25 (PDF, same doc)",
    poller,
    async (push, pushErr) => {
      const tasks = Array.from({ length: 50 }, (_, i) => i);
      await withLimit(tasks, 25, async (i) => {
        const r = await postConvert("pdf", hotBytes);
        push(r.elapsed_ms);
        if (r.status !== 200) pushErr(`c25 #${i}: ${r.status} ${r.error}`);
      });
      return null;
    },
  );

  // 6. Mixed format concurrent
  const mixed_concurrent_10 = await phase(
    "phase 6: mixed format concurrent (50 reqs PDF/HTML/MD at c=10)",
    poller,
    async (push, pushErr) => {
      const formats = ["pdf", "html", "markdown"];
      const tasks = Array.from({ length: 50 }, (_, i) => formats[i % formats.length]);
      await withLimit(tasks, 10, async (fmt, i) => {
        const r = await postConvert(fmt, hotBytes);
        push(r.elapsed_ms);
        if (r.status !== 200) pushErr(`mix-c10 ${fmt} #${i}: ${r.status} ${r.error}`);
      });
      return null;
    },
  );

  poller.stop();

  // Final cross-phase RSS view
  const allMem = memStatsFromSamples(poller.samples);

  const report = {
    target: baseUrl,
    timestamp: new Date().toISOString(),
    corpus_size: corpus.length,
    memory_pids: pids,
    memory_overall: allMem,
    phases: {
      warmup,
      corpus_sweep,
      hot_sequential,
      hot_concurrent_10,
      hot_concurrent_25,
      mixed_concurrent_10,
    },
    corpus_detail: corpusDetail,
  };

  await writeFile(reportPath, JSON.stringify(report, null, 2));
  console.log(`\nreport written: ${reportPath}`);
  console.log(`overall RSS: min=${allMem?.min_mb}MB mean=${allMem?.mean_mb}MB max=${allMem?.max_mb}MB`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});

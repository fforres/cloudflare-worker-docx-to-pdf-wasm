#!/usr/bin/env python3
"""Score one approach against the test corpus.

Usage: score.py <approach-name> <cli-binary-path>

Walks fixtures/{tier1,tier2,tier3}/, runs `<cli> <docx> <out.pdf>` for each,
compares against reference-pdfs/, writes per-doc JSON + summary.md to
results/<approach-name>/.
"""
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures"
REFERENCES = ROOT / "reference-pdfs"
RESULTS_BASE = ROOT / "results"

TIERS = ("tier1", "tier2", "tier3")


def tokenize(text: str) -> set:
    return set(re.findall(r"[a-z0-9]+", text.lower()))


def pdf_text(path: Path) -> str:
    try:
        r = subprocess.run(
            ["pdftotext", "-layout", "-q", str(path), "-"],
            capture_output=True, text=True, timeout=30
        )
        return r.stdout
    except Exception as e:
        return ""


def pdf_pages(path: Path) -> int:
    try:
        r = subprocess.run(
            ["pdfinfo", str(path)],
            capture_output=True, text=True, timeout=30
        )
        m = re.search(r"^Pages:\s+(\d+)", r.stdout, re.MULTILINE)
        return int(m.group(1)) if m else 0
    except Exception:
        return 0


def pdf_image_count(path: Path) -> int:
    try:
        r = subprocess.run(
            ["pdfimages", "-list", str(path)],
            capture_output=True, text=True, timeout=30
        )
        lines = [l for l in r.stdout.splitlines() if l.strip()]
        return max(0, len(lines) - 2)  # subtract header rows
    except Exception:
        return 0


def score_doc(approach: str, cli: Path, docx: Path, reference_pdf: Path, out_pdf: Path) -> dict:
    out_pdf.parent.mkdir(parents=True, exist_ok=True)
    if out_pdf.exists():
        out_pdf.unlink()

    t0 = time.time()
    try:
        proc = subprocess.run(
            [str(cli), str(docx), str(out_pdf)],
            capture_output=True, text=True, timeout=120
        )
        elapsed_ms = int((time.time() - t0) * 1000)
        ok = proc.returncode == 0 and out_pdf.exists() and out_pdf.stat().st_size > 0
        err = proc.stderr[-500:] if proc.stderr else ""
    except subprocess.TimeoutExpired:
        elapsed_ms = 120_000
        ok = False
        err = "timeout"
    except Exception as e:
        elapsed_ms = int((time.time() - t0) * 1000)
        ok = False
        err = str(e)

    result = {
        "approach": approach,
        "doc": str(docx.relative_to(ROOT)),
        "ok": ok,
        "elapsed_ms": elapsed_ms,
        "error": err if not ok else "",
        "gen_pdf_bytes": out_pdf.stat().st_size if out_pdf.exists() else 0,
    }

    if ok and reference_pdf.exists():
        ref_text = pdf_text(reference_pdf)
        gen_text = pdf_text(out_pdf)
        ref_tokens = tokenize(ref_text)
        gen_tokens = tokenize(gen_text)
        if ref_tokens:
            recall = len(ref_tokens & gen_tokens) / len(ref_tokens)
        else:
            recall = 1.0 if not gen_tokens else 0.0
        ref_pages = pdf_pages(reference_pdf)
        gen_pages = pdf_pages(out_pdf)
        page_delta = abs(gen_pages - ref_pages) / max(1, ref_pages)
        result.update({
            "text_recall": round(recall, 3),
            "ref_pages": ref_pages,
            "gen_pages": gen_pages,
            "page_delta": round(page_delta, 3),
            "ref_images": pdf_image_count(reference_pdf),
            "gen_images": pdf_image_count(out_pdf),
        })
    return result


def main():
    if len(sys.argv) != 3:
        print("usage: score.py <approach-name> <cli-binary-path>", file=sys.stderr)
        sys.exit(2)
    approach = sys.argv[1]
    cli = Path(sys.argv[2]).resolve()
    if not cli.exists():
        print(f"CLI not found: {cli}", file=sys.stderr)
        sys.exit(2)

    out_dir = RESULTS_BASE / approach
    out_dir.mkdir(parents=True, exist_ok=True)

    all_results = []
    for tier in TIERS:
        for docx in sorted((FIXTURES / tier).glob("*.docx")):
            ref = REFERENCES / tier / (docx.stem + ".pdf")
            gen = out_dir / tier / (docx.stem + ".pdf")
            r = score_doc(approach, cli, docx, ref, gen)
            r["tier"] = tier
            all_results.append(r)
            status = "OK " if r["ok"] else "FAIL"
            recall = r.get("text_recall", "-")
            print(f"  [{tier}] {status}  recall={recall}  {docx.name}")

    # Aggregate
    summary_lines = [f"# Scorecard: {approach}\n"]
    summary_lines.append("| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |")
    summary_lines.append("|------|-------|----|--------------|--------------|-------------|--------|")
    for tier in TIERS:
        rs = [r for r in all_results if r["tier"] == tier]
        oks = [r for r in rs if r["ok"]]
        scored = [r for r in oks if "text_recall" in r]
        n = len(rs); n_ok = len(oks)
        avg_recall = sum(r["text_recall"] for r in scored) / len(scored) if scored else 0
        avg_page = sum(r["page_delta"] for r in scored) / len(scored) if scored else 0
        avg_img = sum(abs(r["gen_images"] - r["ref_images"]) for r in scored) / len(scored) if scored else 0
        avg_ms = sum(r["elapsed_ms"] for r in oks) / len(oks) if oks else 0
        summary_lines.append(
            f"| {tier} | {n} | {n_ok} | {avg_recall:.2f} | {avg_page:.2f} | {avg_img:.1f} | {avg_ms:.0f} |"
        )

    summary_lines.append("\n## Per-document results\n")
    for r in all_results:
        if r["ok"]:
            summary_lines.append(
                f"- [{r['tier']}] **{Path(r['doc']).name}** — recall={r.get('text_recall','-')} "
                f"pages={r.get('gen_pages','-')}/{r.get('ref_pages','-')} "
                f"imgs={r.get('gen_images','-')}/{r.get('ref_images','-')} "
                f"size={r['gen_pdf_bytes']}B time={r['elapsed_ms']}ms"
            )
        else:
            err_brief = r["error"].splitlines()[0] if r["error"] else "no output"
            summary_lines.append(f"- [{r['tier']}] **{Path(r['doc']).name}** — FAIL: `{err_brief}`")

    (out_dir / "summary.md").write_text("\n".join(summary_lines) + "\n")
    (out_dir / "results.json").write_text(json.dumps(all_results, indent=2))
    print(f"\nWrote {out_dir}/summary.md and results.json")


if __name__ == "__main__":
    main()

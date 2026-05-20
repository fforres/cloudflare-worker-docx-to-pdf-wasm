#!/usr/bin/env python3
"""One-off scorer for the 'complex' tier of real-world DOCX files.

Identical to score.py but only iterates fixtures/complex/ and reference-pdfs/complex/.
Writes results to results/<approach>/.

Usage: score_complex.py <approach-name> <cli-binary-path>
"""
import json
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures" / "complex"
REFERENCES = ROOT / "reference-pdfs" / "complex"
RESULTS_BASE = ROOT / "results"


def tokenize(text: str) -> set:
    return set(re.findall(r"[a-z0-9]+", text.lower()))


def pdf_text(path: Path) -> str:
    try:
        r = subprocess.run(
            ["pdftotext", "-layout", "-q", str(path), "-"],
            capture_output=True, text=True, timeout=60
        )
        return r.stdout
    except Exception:
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
        return max(0, len(lines) - 2)
    except Exception:
        return 0


def score_doc(approach, cli, docx, reference_pdf, out_pdf):
    out_pdf.parent.mkdir(parents=True, exist_ok=True)
    if out_pdf.exists():
        out_pdf.unlink()

    t0 = time.time()
    try:
        proc = subprocess.run(
            [str(cli), str(docx), str(out_pdf)],
            capture_output=True, text=True, timeout=180
        )
        elapsed_ms = int((time.time() - t0) * 1000)
        ok = proc.returncode == 0 and out_pdf.exists() and out_pdf.stat().st_size > 0
        err = proc.stderr[-500:] if proc.stderr else ""
    except subprocess.TimeoutExpired:
        elapsed_ms = 180_000
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
        "src_docx_bytes": docx.stat().st_size,
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
        print("usage: score_complex.py <approach-name> <cli-binary-path>", file=sys.stderr)
        sys.exit(2)
    approach = sys.argv[1]
    cli = Path(sys.argv[2]).resolve()
    if not cli.exists():
        print(f"CLI not found: {cli}", file=sys.stderr)
        sys.exit(2)

    out_dir = RESULTS_BASE / approach
    out_dir.mkdir(parents=True, exist_ok=True)

    all_results = []
    for docx in sorted(FIXTURES.glob("*.docx")):
        ref = REFERENCES / (docx.stem + ".pdf")
        gen = out_dir / "complex" / (docx.stem + ".pdf")
        r = score_doc(approach, cli, docx, ref, gen)
        r["tier"] = "complex"
        all_results.append(r)
        status = "OK " if r["ok"] else "FAIL"
        recall = r.get("text_recall", "-")
        pages = f"{r.get('gen_pages','-')}/{r.get('ref_pages','-')}"
        print(f"  [complex] {status}  recall={recall} pages={pages}  {docx.name}")

    summary_lines = [f"# Scorecard: {approach} (complex tier)\n"]
    summary_lines.append("| Tier | Files | OK | Recall (avg) | Page Δ (avg) | Img Δ (avg) | Avg ms |")
    summary_lines.append("|------|-------|----|--------------|--------------|-------------|--------|")
    rs = all_results
    oks = [r for r in rs if r["ok"]]
    scored = [r for r in oks if "text_recall" in r]
    n, n_ok = len(rs), len(oks)
    avg_recall = sum(r["text_recall"] for r in scored) / len(scored) if scored else 0
    avg_page = sum(r["page_delta"] for r in scored) / len(scored) if scored else 0
    avg_img = sum(abs(r["gen_images"] - r["ref_images"]) for r in scored) / len(scored) if scored else 0
    avg_ms = sum(r["elapsed_ms"] for r in oks) / len(oks) if oks else 0
    summary_lines.append(
        f"| complex | {n} | {n_ok} | {avg_recall:.2f} | {avg_page:.2f} | {avg_img:.1f} | {avg_ms:.0f} |"
    )

    summary_lines.append("\n## Per-document results\n")
    for r in all_results:
        if r["ok"]:
            summary_lines.append(
                f"- **{Path(r['doc']).name}** ({r['src_docx_bytes']//1024} KB src) — "
                f"recall={r.get('text_recall','-')} "
                f"pages={r.get('gen_pages','-')}/{r.get('ref_pages','-')} "
                f"imgs={r.get('gen_images','-')}/{r.get('ref_images','-')} "
                f"out={r['gen_pdf_bytes']//1024}KB time={r['elapsed_ms']}ms"
            )
        else:
            err_brief = r["error"].splitlines()[0] if r["error"] else "no output"
            summary_lines.append(f"- **{Path(r['doc']).name}** ({r['src_docx_bytes']//1024} KB src) — FAIL: `{err_brief}`")

    (out_dir / "summary.md").write_text("\n".join(summary_lines) + "\n")
    (out_dir / "results.json").write_text(json.dumps(all_results, indent=2))
    print(f"\nWrote {out_dir}/summary.md and results.json")


if __name__ == "__main__":
    main()

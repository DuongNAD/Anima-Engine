#!/usr/bin/env python3
"""Oracle cho dataflow audit.

Agent xuất dataflow.json (claim). cargo-modules xuất dataflow-truth.dot (sự thật).
Script này đối chiếu. Edge bịa -> fail. Edge có thật mà agent bỏ sót -> fail.
Không có bước này thì "phân tích luồng dữ liệu" chỉ là văn xuôi không kiểm được.
"""
import json
import re
import sys
from pathlib import Path


def load_claim(p: Path):
    d = json.loads(p.read_text())
    edges = set()
    for e in d.get("edges", []):
        for k in ("from", "to", "via", "invariant"):
            if k not in e:
                sys.exit(f"FAIL: edge thieu truong '{k}': {e}")
        if not re.match(r"^.+\.rs:\d+$", e["via"]):
            sys.exit(f"FAIL: 'via' phai la file.rs:line, nhan duoc {e['via']!r}")
        edges.add((e["from"], e["to"]))
    return edges, d


def load_truth(p: Path):
    edges = set()
    for line in p.read_text().splitlines():
        m = re.search(r'"([^"]+)"\s*->\s*"([^"]+)"', line)
        if m:
            edges.add((m.group(1).split("::")[-1], m.group(2).split("::")[-1]))
    return edges


def main():
    if len(sys.argv) != 3:
        sys.exit("dung: verify_dataflow.py <dataflow.json> <dataflow-truth.dot>")
    claim_p, truth_p = Path(sys.argv[1]), Path(sys.argv[2])
    for p in (claim_p, truth_p):
        if not p.exists():
            sys.exit(f"FAIL: thieu {p}")

    claim, raw = load_claim(claim_p)
    truth = load_truth(truth_p)

    invented = claim - truth
    missed = truth - claim
    rc = 0

    if invented:
        print(f"FAIL: {len(invented)} edge BIA (co trong claim, khong co that):")
        for a, b in sorted(invented):
            print(f"  {a} -> {b}")
        rc = 1
    if missed:
        print(f"FAIL: {len(missed)} edge BO SOT (co that, agent khong bao):")
        for a, b in sorted(list(missed)[:20]):
            print(f"  {a} -> {b}")
        rc = 1

    # Mỗi edge phải khai một invariant kiểm được — "data flows" không tính.
    vague = [e for e in raw.get("edges", [])
             if len(e["invariant"].split()) < 3 or "flow" in e["invariant"].lower()]
    if vague:
        print(f"FAIL: {len(vague)} edge co invariant rong tuech:")
        for e in vague:
            print(f"  {e['from']}->{e['to']}: {e['invariant']!r}")
        rc = 1

    if rc == 0:
        print(f"QUA CONG — {len(claim)} edge khop hoan toan voi cargo-modules")
    return rc


if __name__ == "__main__":
    sys.exit(main())

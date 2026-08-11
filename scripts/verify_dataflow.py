#!/usr/bin/env python3
"""Oracle cho dataflow audit.

Agent xuất dataflow.json (claim). cargo-modules xuất dataflow-truth.dot (sự thật).
Script này đối chiếu. Edge bịa -> fail. Edge có thật mà agent bỏ sót -> fail.
Không có bước này thì "phân tích luồng dữ liệu" chỉ là văn xuôi không kiểm được.

HAI PHÉP LỌC BẮT BUỘC, đo được trên repo thật (Anima-Engine):

    tổng cạnh thô                    788
    bỏ "owns" (phân cấp module)      502
    gộp về mức module, bỏ tự trỏ      11

788 cạnh thì không agent nào liệt kê nổi — bắt liệt kê đủ là ra một task bất khả
thi, và oracle sẽ luôn fail bất kể agent làm tốt đến đâu. 11 cạnh thì vừa sức,
và đó mới là bản đồ kiến trúc thật sự đáng đọc.

- "owns" là quan hệ chứa của cây module, KHÔNG phải luồng dữ liệu.
- Cạnh mức item (`a::b::hàm_x -> c::d::hàm_y`) quá mịn; gộp lên module cho ra
  câu hỏi kiến trúc thật: module nào phụ thuộc module nào.
"""
import json
import re
import sys
from pathlib import Path

# Số cấp giữ lại sau tên crate. 1 = "core", 2 = "core::ecs".
DEPTH = int(sys.argv[3]) if len(sys.argv) > 3 else 1


def module_of(path: str) -> str:
    """`anima_engine_lib::core::ecs::Foo` -> `core` (DEPTH=1)."""
    parts = path.split("::")
    if len(parts) == 1:
        return parts[0]
    return "::".join(parts[1 : 1 + DEPTH])


def load_truth(p: Path):
    edges = set()
    for line in p.read_text().splitlines():
        if 'label="uses"' not in line:
            continue
        m = re.search(r'"([^"]+)"\s*->\s*"([^"]+)"', line)
        if not m:
            continue
        a, b = module_of(m.group(1)), module_of(m.group(2))
        if a and b and a != b:
            edges.add((a, b))
    return edges


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


def main():
    if len(sys.argv) < 3:
        sys.exit("dung: verify_dataflow.py <dataflow.json> <dataflow-truth.dot> [depth]")
    claim_p, truth_p = Path(sys.argv[1]), Path(sys.argv[2])
    for p in (claim_p, truth_p):
        if not p.exists():
            sys.exit(f"FAIL: thieu {p}")

    claim, raw = load_claim(claim_p)
    truth = load_truth(truth_p)
    if not truth:
        sys.exit("FAIL: khong doc duoc canh 'uses' nao tu file su that. "
                 "Chay lai 'just dataflow-truth'.")

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
        for a, b in sorted(missed):
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
        cyc = sorted((a, b) for a, b in claim if (b, a) in claim and a < b)
        if cyc:
            print(f"\nLUU Y: {len(cyc)} cap phu thuoc VONG giua cac module:")
            for a, b in cyc:
                print(f"  {a} <-> {b}")
            print("Khong phai loi, nhung dang ghi vao FINDINGS.md dang SMELL.")
    return rc


if __name__ == "__main__":
    sys.exit(main())

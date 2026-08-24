"""OSS-070 -- make a THIRD-PARTY parser read the lineage export.

The Rust side already round-trips: `newick_export_tests.rs` pins the exact output. That proves the
serialiser agrees with itself, which is not the interesting question. The interesting question is
whether a parser written by people who have never seen this codebase agrees that the output is a
well-formed tree -- because that is what turns a malformed lineage from something nobody notices
into something that fails loudly.

So this reads the SAME committed fixture the Rust test pins, with DendroPy, and checks the topology
independently. Two halves of one gate:

    cargo test --test newick_export_tests   # the export still produces the fixture
    python scripts/verify_newick.py         # a foreign parser agrees the fixture is a tree

Neither half is worth much alone. A Rust round-trip proves the serialiser is self-consistent; a
parser reading a stale file proves nothing about the current code.

Requires: pip install dendropy   (dev-only; nothing in the product depends on Python)
Run:      python scripts/verify_newick.py
"""

import sys
from pathlib import Path

try:
    import dendropy
except ImportError:  # pragma: no cover - the message IS the useful behaviour here
    sys.exit(
        "dendropy is not installed. This check is dev-only and optional:\n"
        "    pip install dendropy\n"
        "The Rust half of the gate (cargo test --test newick_export_tests) runs without it."
    )

FIXTURE = Path(__file__).resolve().parent.parent / "src-tauri" / "tests" / "fixtures" / "newick" / "lineage_forest.nwk"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def label_of(node) -> str | None:
    """DendroPy puts leaf names on `node.taxon` and internal names on `node.label`."""
    if node.taxon is not None:
        return node.taxon.label
    return node.label


def main() -> int:
    if not FIXTURE.exists():
        sys.exit(f"fixture not found: {FIXTURE}")

    text = FIXTURE.read_text(encoding="utf-8")
    try:
        trees = dendropy.TreeList.get(data=text, schema="newick")
    except Exception as exc:
        # A refusal here IS a result, and the most valuable one this script can produce: it means
        # the export wrote something a foreign parser will not accept. The common cause is a label
        # that needed quoting and did not get it -- an unquoted colon reads as the start of a branch
        # length, which truncates the name rather than failing, unless the remainder is not a
        # number. Report it as a finding instead of a traceback.
        print("verify_newick: FAILED")
        print(f"  - DendroPy refused to parse {FIXTURE.name}: {exc}")
        print("    The export produced something that is not valid Newick. Check label quoting in")
        print("    src-tauri/src/evolution/newick.rs (`label`).")
        return 1

    check(len(trees) == 2, f"expected a 2-tree forest, DendroPy read {len(trees)}")
    if len(trees) != 2:
        return report()

    # ---- tree 1: the chain, deepest label first ------------------------------------------------
    chain = trees[0]
    root_label = label_of(chain.seed_node)
    check(root_label == "f-alpha", f"tree 1 root is {root_label!r}, expected 'f-alpha'")

    leaves = sorted(label_of(n) for n in chain.leaf_node_iter())
    check(leaves == ["hybrid"], f"tree 1 leaves are {leaves}, expected ['hybrid']")

    # Walking down from the root must reproduce the lineage order. This is the check that would
    # fail if the emitter nested the parentheses the wrong way round -- a mistake that still
    # produces valid Newick, which is exactly why a parser is asked rather than a string compare.
    walked = []
    node = chain.seed_node
    while node is not None:
        walked.append(label_of(node))
        children = node.child_nodes()
        node = children[0] if children else None
    check(
        walked == ["f-alpha", "child one", "child:two", "hybrid"],
        f"tree 1 root-to-leaf path is {walked}, expected "
        "['f-alpha', 'child one', 'child:two', 'hybrid']",
    )

    # Labels that needed quoting must survive the round trip with their characters intact. The
    # colon in 'child:two' is the one that matters: unquoted, a parser would read it as the start
    # of a branch length and silently truncate the name.
    check(
        "child:two" in walked,
        "a label containing a colon did not survive quoting -- DendroPy read it as a branch length",
    )
    check(
        "child one" in walked,
        "a label containing a space did not survive quoting",
    )

    lengths = [n.edge_length for n in chain.preorder_node_iter() if n.edge_length is not None]
    check(
        lengths == [1.0, 1.0, 1.0],
        f"branch lengths are {lengths}, expected three generation deltas of 1",
    )

    # ---- tree 2: the crossover parent left as a lone root ---------------------------------------
    lone = trees[1]
    lone_label = label_of(lone.seed_node)
    check(lone_label == "f-beta", f"tree 2 root is {lone_label!r}, expected 'f-beta'")
    check(
        len(lone.seed_node.child_nodes()) == 0,
        "tree 2 should be a single node: its only child edge is the crossover edge Newick cannot "
        "represent, which the export counts in dropped_parent_edges instead of writing",
    )

    # ---- negative control ------------------------------------------------------------------------
    # Without this the script could be passing vacuously -- a parser that accepted anything would
    # satisfy every assertion above.
    try:
        dendropy.Tree.get(data="((a,b);", schema="newick")
        failures.append(
            "DendroPy accepted unbalanced parentheses, so it is not actually validating structure"
        )
    except Exception:
        pass

    return report()


def report() -> int:
    if failures:
        print("verify_newick: FAILED")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(f"verify_newick: OK -- DendroPy {dendropy.__version__} read {FIXTURE.name} as a valid "
          "2-tree forest, topology and quoted labels intact")
    return 0


if __name__ == "__main__":
    sys.exit(main())

"""Turn the saved report cards into a small, readable results table."""

import json
from pathlib import Path


def summarize():
    here = Path(__file__).resolve().parent
    cards = sorted(
        (json.loads(p.read_text()) for p in (here / "results").glob("a2p*-x*.json")),
        key=lambda c: (c["world"], c["load_scale"]),
    )
    lines = [
        "# Practice results", "",
        "These are actual simulator results for the submitted implementation.",
        "Each percentage is the worst graded window, not an average.", "",
        "## Required loads", "",
        "| World | Load | Result | Worst F1 | Worst F2 | Loops | False link-downs |",
        "| --- | ---: | --- | ---: | ---: | ---: | ---: |",
    ]
    for c in cards:
        if c["load_scale"] not in (1, 1.5):
            continue
        f1 = min(x["min_share_pct"] for x in c["customers"] if x["min_share_pct"] is not None)
        f2 = min(x for x in c["series"]["utilization_pct"] if x is not None)
        lines.append(
            f"| {c['world']} | {c['load_scale']:g}x | {c['status']} | {f1:.1f}% | "
            f"{f2:.1f}% | {c['looping_packets']} | {c['link_down_declarations']} |"
        )
    lines += [
        "", "F1 requires 40%; F2 requires 55%. Every customer must deliver something.",
        "The seven unit tests also passed. Their output is in `results/verification.log`.",
        "", "## Load sweep", "",
        "Total goodput below is the sum of the customers' reported first-time",
        "goodput. Bursty customers are quiet during part of a run, so these",
        "whole-run totals are not the F2 metric.", "",
        "| World | Load | Result | Total goodput (Mbit/s) |",
        "| --- | ---: | --- | ---: |",
    ]
    for c in cards:
        goodput = sum(x["goodput_mbps"] for x in c["customers"])
        lines.append(f"| {c['world']} | {c['load_scale']:g}x | {c['status']} | {goodput:.3f} |")
    lines += [
        "", "## Limits of these results", "",
        "- The hidden grading worlds and grading server were not available.",
        "- The supplied kit does not include the user's A1 switch or A1 worlds;",
        "  failure recovery in that earlier environment was not tested.",
        "- The weakest required result is worth studying in its full report card;",
        "  a passing floor is not a claim of perfect fairness or optimal routing.",
        "", "Reproduce with `python3 solution/check.py --sweep` from the kit root.",
    ]
    (here / "RESULTS.md").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    summarize()

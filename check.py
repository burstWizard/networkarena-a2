#!/usr/bin/env python3
"""Build and test the switch. Optional --sweep adds the 0.5x to 2x load curve."""

import argparse
import csv
import json
from pathlib import Path
import subprocess
import sys
from summarize import summarize

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
WASM = HERE / "target/wasm32-unknown-unknown/release/fair_switch.wasm"
SIM = ROOT / "target/release/competitive_net_sim"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sweep", action="store_true")
    args = parser.parse_args()
    for command in [
        ["cargo", "build", "--release", "--locked"],
        ["cargo", "test", "--manifest-path", str(HERE / "Cargo.toml"), "--locked"],
        ["cargo", "build", "--manifest-path", str(HERE / "Cargo.toml"),
         "--release", "--target", "wasm32-unknown-unknown", "--locked"],
    ]:
        subprocess.run(command, cwd=ROOT, check=True)

    results = HERE / "results"
    results.mkdir(exist_ok=True)
    loads = [0.5, 0.75, 1, 1.25, 1.5, 2] if args.sweep else [1, 1.5]
    rows = []
    required_passed = True
    for world in sorted((ROOT / "worlds/practice").glob("a2p*.toml")):
        for load in loads:
            stem = f"{world.stem}-x{load}"
            json_path = results / f"{stem}.json"
            # Prevent a failed run from accidentally reusing an old card.
            json_path.unlink(missing_ok=True)
            with (results / f"{stem}.txt").open("w") as log:
                run = subprocess.run(
                    [str(SIM), "run-world", str(world), "--program", str(WASM),
                     "--score", "--load-scale", str(load), "--json", str(json_path)],
                    cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                )
            if not json_path.exists():
                raise RuntimeError(f"No report card: read results/{stem}.txt")
            card = json.loads(json_path.read_text())
            passed = run.returncode == 0 and card["passed"]
            if load in [1, 1.5]:
                required_passed &= passed
            f1 = min(c["min_share_pct"] for c in card["customers"]
                     if c["min_share_pct"] is not None)
            row = {
                "world": world.stem, "load": load, "passed": passed,
                "minimum_customer_share_pct": round(f1, 2),
                "total_goodput_mbps": round(sum(c["goodput_mbps"] for c in card["customers"]), 4),
                "loops": card["looping_packets"],
                "false_link_down": card["link_down_declarations"],
            }
            rows.append(row)
            print(f"{stem:25} {'PASS' if passed else 'FAIL'}  worst share {f1:.1f}%", flush=True)
    with (results / "summary.csv").open("w", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)
    # A stable, conveniently named copy of the exact tested artifact.
    (HERE / "fair_switch.wasm").write_bytes(WASM.read_bytes())
    summarize()
    print(f"Required runs: {'all passed' if required_passed else 'FAILURES'}")
    return 0 if required_passed else 1


if __name__ == "__main__":
    sys.exit(main())

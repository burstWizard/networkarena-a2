# Practice results

These are actual simulator results for the submitted implementation.
Each percentage is the worst graded window, not an average.

## Required loads

| World | Load | Result | Worst F1 | Worst F2 | Loops | False link-downs |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| a2p1-dumb-004 | 1x | PASS | 86.6% | 89.8% | 0 | 0 |
| a2p1-dumb-004 | 1.5x | PASS | 86.7% | 91.0% | 0 | 0 |
| a2p1-grid-009 | 1x | PASS | 63.5% | 91.3% | 0 | 0 |
| a2p1-grid-009 | 1.5x | PASS | 61.7% | 88.2% | 0 | 0 |
| a2p1-ring-020 | 1x | PASS | 93.4% | 95.8% | 0 | 0 |
| a2p1-ring-020 | 1.5x | PASS | 92.8% | 95.9% | 0 | 0 |
| a2p2-dumb-003 | 1x | PASS | 68.9% | 82.7% | 0 | 0 |
| a2p2-dumb-003 | 1.5x | PASS | 65.7% | 90.1% | 0 | 0 |
| a2p2-grid-006 | 1x | PASS | 72.8% | 84.1% | 0 | 0 |
| a2p2-grid-006 | 1.5x | PASS | 83.2% | 97.5% | 0 | 0 |

F1 requires 40%; F2 requires 55%. Every customer must deliver something.
The seven unit tests also passed. Their output is in `results/verification.log`.

## Load sweep

Total goodput below is the sum of the customers' reported first-time
goodput. Bursty customers are quiet during part of a run, so these
whole-run totals are not the F2 metric.

| World | Load | Result | Total goodput (Mbit/s) |
| --- | ---: | --- | ---: |
| a2p1-dumb-004 | 0.5x | PASS | 12.580 |
| a2p1-dumb-004 | 0.75x | PASS | 14.366 |
| a2p1-dumb-004 | 1x | PASS | 16.145 |
| a2p1-dumb-004 | 1.25x | PASS | 17.927 |
| a2p1-dumb-004 | 1.5x | PASS | 19.398 |
| a2p1-dumb-004 | 2x | PASS | 20.540 |
| a2p1-grid-009 | 0.5x | PASS | 12.126 |
| a2p1-grid-009 | 0.75x | PASS | 13.317 |
| a2p1-grid-009 | 1x | PASS | 14.458 |
| a2p1-grid-009 | 1.25x | PASS | 15.627 |
| a2p1-grid-009 | 1.5x | PASS | 16.792 |
| a2p1-grid-009 | 2x | PASS | 19.139 |
| a2p1-ring-020 | 0.5x | PASS | 14.291 |
| a2p1-ring-020 | 0.75x | PASS | 16.506 |
| a2p1-ring-020 | 1x | PASS | 18.765 |
| a2p1-ring-020 | 1.25x | PASS | 20.994 |
| a2p1-ring-020 | 1.5x | PASS | 20.582 |
| a2p1-ring-020 | 2x | PASS | 23.047 |
| a2p2-dumb-003 | 0.5x | PASS | 8.977 |
| a2p2-dumb-003 | 0.75x | PASS | 9.540 |
| a2p2-dumb-003 | 1x | PASS | 12.020 |
| a2p2-dumb-003 | 1.25x | PASS | 12.958 |
| a2p2-dumb-003 | 1.5x | PASS | 14.201 |
| a2p2-dumb-003 | 2x | PASS | 16.775 |
| a2p2-grid-006 | 0.5x | PASS | 11.099 |
| a2p2-grid-006 | 0.75x | PASS | 12.368 |
| a2p2-grid-006 | 1x | PASS | 13.754 |
| a2p2-grid-006 | 1.25x | PASS | 14.879 |
| a2p2-grid-006 | 1.5x | PASS | 15.804 |
| a2p2-grid-006 | 2x | PASS | 15.469 |

## Limits of these results

- The hidden grading worlds and grading server were not available.
- The supplied kit does not include the user's A1 switch or A1 worlds;
  failure recovery in that earlier environment was not tested.
- The weakest required result is worth studying in its full report card;
  a passing floor is not a claim of perfect fairness or optimal routing.

Reproduce with `python3 solution/check.py --sweep` from the kit root.

# Design card: Make It Fair

## 1. One queue per customer

Queue 0 carries control packets and ACKs. Each customer gets another queue,
selected by its source prefix. AIMD and rate-matching customers never share a
data queue.

This matters because AIMD halves its rate after loss, while rate matching
keeps probing at roughly 1.25 times its acknowledged rate. If they shared one
queue, the more aggressive sender could repeatedly crowd out the AIMD sender.

## 2. Equal byte service with DRR

Every queue has DRR weight 1. A busy customer receives one full-size packet's
worth of credit per round. A customer sending 200-byte packets can send several
within that credit. Fairness is therefore in bytes, rather than packet counts.

Empty queues are skipped. Small customers use what they need; busy customers
use the remaining service. This approximates max-min sharing at each port.
It is a local scheduler, not an exact end-to-end max-min optimizer.

## 3. Keep queues bounded

Let `N` be the number of customer/port pairs on the paths currently assigned
through this switch. The basic allowance is `30 / max(N, 1)` full-size packets,
limited to between 2 and 12.

| Queue | Drop line | Reason |
| --- | --- | --- |
| Control and ACKs | 2 packets | Separate space for small, essential packets |
| AIMD customer | Basic allowance | Absorb short bursts and avoid unnecessary rate halvings |
| Rate-matching customer | At most 4 packets | Limit its persistent queue and leave room for others |
| Customer not assigned to this port | 1 packet | A small allowance during route changes |

The hard cap is one packet above the drop line, so normal intentional drops
are reported as policy drops. The simulator still enforces the shared
40,000-byte pool.

These limits can overcommit the pool, especially during a route change or
with many customer/port pairs. They are not strict reservations. That choice
lets busy queues use space when others are quiet, at the cost of possible
pool drops. Queue 0 uses DRR too; it is isolated but does not have strict
priority. Short data queues limit how long it waits.

## 4. Learn the network and share load reports

HELLO packets discover neighbours. The apps' initial zero-byte announcements
teach the switch which hosts are local. Reports flood those locations,
link capacities, assigned paths, and recent drop information through the network.
Sequence numbers stop the same report from circulating repeatedly.

Drops are summarized every 500 ms. The report remembers the worst of the last
two samples and uses levels 0, 1, 2, and 3 for 0, 1-4, 5-24, and 25+ drops.
This follows the supplied `load_report` example, with shorter memory.

The switch declares a port up when it learns its neighbour or local host.
It does not mistake queue drops for a failed link. A2 has no link failures,
so this version never declares a link down.

## 5. Choose one path per customer

The source switch considers simple paths up to two hops longer than the
shortest. Enumeration is bounded at 128 paths. It estimates the customer's
share on every link from capacity, customer contracts, and advertised paths.
Small demands are satisfied first; larger demands split what remains.

A path's score is its smallest estimated share, with a small penalty for
reported drops. The switch picks the highest score, then the shorter path
on a tie. It checks again every 500 ms, staggered across switches.

A move needs more than a 25% improvement and at least two seconds since the
previous move. This prevents small report changes from causing oscillation.
The policy is deliberately conservative: it counts each assigned customer's
full contract even while a burst is quiet. DRR can immediately use spare
service, but routing may leave useful burst-specific rearrangements unexplored.
The small drop penalty alone normally does not trigger a move.

## 6. Change paths without mixing them

Each new path gets a version number. The source advertises the path and waits
200 ms before tagging new data packets with that version in `custom0`.
Every switch forwards that version along the same simple path.

Old packets retain their old version, and old entries stay for two seconds
to drain. They are then removed. A packet with an unknown version is punted
and lost; it never falls back onto an unrelated path. Thus a delayed control
update can lose packets without creating a forwarding loop.

ACKs use ordinary shortest paths and queue 0. No customer packets are copied
or fabricated by the controller.

## What to be ready to explain

- Deeper queues absorb bursts but increase delay and can cause retransmissions.
- Earlier drops save buffer space but can hold AIMD below its useful rate.
- DRR provides byte fairness and lets unused service go to other customers.
- Whole paths avoid per-packet path changes; versioning prevents mixed-route loops.
- Conservative routing is stable, but it does not guarantee the globally best allocation.

See `RESULTS.md` for measured performance. The practice results establish what
this implementation passed; they do not establish a result on hidden worlds.

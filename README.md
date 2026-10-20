# Assignment 2: Fair Switch

The completed switch is in **`fair_switch.wasm`**. The code uses the normal SDK;
the simulator, scoring rules, and practice worlds have not been changed.

## Run it

From the kit's main directory:

```sh
./target/release/competitive_net_sim run-world \
  worlds/practice/a2p2-dumb-003.toml \
  --program solution/fair_switch.wasm --score
```

Build it again:

```sh
rustup target add wasm32-unknown-unknown
cargo build --manifest-path solution/Cargo.toml \
  --release --target wasm32-unknown-unknown --locked
```

The build produces `solution/target/wasm32-unknown-unknown/release/fair_switch.wasm`.

## Check everything

```sh
python3 solution/check.py           # unit tests and all 10 required practice runs
python3 solution/check.py --sweep   # also try 0.5, 0.75, 1.25, and 2 times the load
```

The script builds both programs, saves the actual report cards in `results/`,
and copies the tested binary to `solution/fair_switch.wasm`. It exits with an
error if a required practice run fails. Rust/Cargo and Python 3 are needed.

## Read the solution

| File | What it contains |
| --- | --- |
| `DESIGN.md` | Short design card and the trade-offs to understand |
| `src/lib.rs` | Discovery, load reports, queue configuration, and route selection |
| `src/pipeline.tiny` | The five-stage packet forwarding program |
| `src/tests.rs` | Seven tests for the main routing and resource-management decisions |
| `RESULTS.md` | Results from the final runs, including both graded load settings |
| `results/` | Full text and JSON evidence |

Start with `DESIGN.md`, then read `choose_routes`, `configure_queues`, and
`on_timer`. The main idea is: give each customer its own queue, share bytes
fairly, and choose a whole path with enough capacity.

This is an A2 solution for worlds without link failures. Hidden grading worlds
were not available, and no graded server attempt has been submitted.

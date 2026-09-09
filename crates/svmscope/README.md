# svmscope

Decode, replay and mutate Solana transactions in [LiteSVM](https://github.com/LiteSVM/litesvm).

Start from a mainnet signature. The crate fetches the transaction and everything it touched, rebuilds that world inside LiteSVM, and lets you run it again as often as you like, offline: decode the CPI tree with every instruction, account and argument named from IDLs; replay the real program binaries against the reconstructed state; change an account by field name, warp the clock or flip a feature gate and replay again; step through the transaction instruction by instruction; freeze the whole world into a fixture that replays deterministically in CI.

```sh
cargo add --dev svmscope
```

```rust,no_run
use svmscope::{Mutation, Scope};

let scope = Scope::new("https://api.mainnet-beta.solana.com");
let sig = "your transaction signature";

let analysis = scope.analyze(sig)?;
println!("{} top-level instructions", analysis.cpi_tree.len());

let mut replay = scope.replay(sig)?;
println!("replay success: {}", replay.run()?.result.success);

replay.advance_seconds(30 * 86_400);
let what_if = replay.simulate(&[Mutation::lamports("SomeAccount111...", 0)])?;
println!("mutated replay success: {}", what_if.result.success);
# Ok::<(), svmscope::Error>(())
```

Enable the `profiler` feature for per-function compute attribution through LiteSVM's register tracing.

See the crate documentation for the step debugger, fixtures, historical reconstruction and the IDL-driven transaction builder.

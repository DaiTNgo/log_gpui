//! `memory_budget_probe` with mimalloc as the global allocator, as the app
//! runs (`src/main.rs`): the memory of a file let go goes back to the
//! system through `release_freed_memory`. Same arguments:
//! `cargo run --release --example mimalloc_probe -- [lines] [--budget N | --cycle]`.
//! (Its "malloc in use" lines read macOS malloc's zones, which mimalloc
//! does not use: only footprint and RSS mean something here.)

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[path = "memory_budget_probe.rs"]
mod probe;

fn main() {
    probe::main();
}

// Hides the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Faster allocations, and memory given back to the system after the peaks (a data build,
/// the bets' simulated boosters), which the Windows heap tends to keep.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    unsealed_app_lib::run()
}

mod api;
mod color;
mod display;
mod effects;
mod engine;
mod geometry;
mod persist;
mod state;
mod ws;

fn main() {
    println!("moodlightpi v{}", env!("CARGO_PKG_VERSION"));
}

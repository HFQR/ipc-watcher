//! This example would only run properly when `watched.rs` example is already started and running.

use ipc_watcher::{shared_memory_open, Watcher};

struct State([u8; 8]);

fn main() {
    let mut mem = shared_memory_open("./example_watched", 36).unwrap();

    let watched = Watcher::<State>::new_from_mem(&mut mem);

    watched.read(|state| println!("Watched State is : {:?}", state.0));
}

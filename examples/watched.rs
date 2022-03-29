//! Start `watched.rs` example first and `watcher.rs` example after to observe the shared state.

use std::sync::mpsc::sync_channel;

use ipc_watcher::{shared_memory_create, Watched};

#[derive(Clone, Copy)]
struct State([u8; 8]);

fn main() {
    let mut mem = shared_memory_create("./example_watched", 36).unwrap();

    let watched = Watched::<State>::new_from_mem(&mut mem);

    watched.write(State([123; 8]));

    let (tx, rx) = sync_channel::<()>(1);

    println!("Waiting for Ctrl + C to exit example...");

    let _ = ctrlc::set_handler(move || {
        let _ = tx.send(());
    });

    let _ = rx.recv();

    drop(watched);
}

mod error;
mod shared;
mod tick;

use std::{mem, path::Path};

use shared_memory::{Shmem, ShmemConf};

use crate::{error::Error, shared::Shared};

/// Create shared memory with given path and size.
/// *. If the path is already exist it would OVERWRITE the original file.
pub fn shared_memory_create(path: impl AsRef<Path>, size: usize) -> Result<Shmem, Error> {
    let mut mem = ShmemConf::new()
        .size(size)
        .force_create_flink()
        .flink(path.as_ref())
        .create()?;
    assert!(mem.set_owner(true));
    Ok(mem)
}

/// Open a shared memory with given path and size.
pub fn shared_memory_open(path: impl AsRef<Path>, size: usize) -> Result<Shmem, Error> {
    let mem = ShmemConf::new().size(size).flink(path.as_ref()).open()?;
    Ok(mem)
}

pub struct Watched<'a, T>(Shared<'a, T>);

impl<T> Drop for Watched<'_, T> {
    fn drop(&mut self) {
        self.0.tick.close();
    }
}

impl<'a, T> Watched<'a, T> {
    /// Construct a new watched value in given [Shmem].
    pub fn new_from_mem(mem: &'a mut Shmem) -> Self {
        let shared = Shared::new_from_mem(mem);
        shared.tick.store(0);
        Self(shared)
    }

    /// Obtain a write lock and write a new `T` to the watchable value.
    pub fn write(&self, value: T) {
        let mut guard = self.0.lock.lock().unwrap();

        // SAFETY:
        // This cast is safe. Watcher<T> type is the only type constructor expose.
        let val = unsafe { mem::transmute::<_, &mut T>(&mut **guard) };
        *val = value;

        self.0.tick.tick();
    }
}

pub struct Watcher<'a, T> {
    tick: u8,
    shared: Shared<'a, T>,
}

impl<'a, T> Watcher<'a, T> {
    /// Construct a new observer from given [Shmem].
    /// The given [Shmem] must contain an already initialized [Watched] value.
    pub fn new_from_mem(mem: &'a mut Shmem) -> Self {
        let shared = Shared::exist_from_mem(mem);

        Watcher { tick: 0, shared }
    }

    /// Obtain a read lock and access &T through a closure.
    /// Closure is expected to be non blocking and kept as shortest in execute time as possible.
    pub fn read<F, O>(&self, func: F) -> O
    where
        F: FnOnce(&T) -> O,
    {
        let guard = self.shared.lock.rlock().unwrap();
        let val = unsafe { mem::transmute::<_, &T>(&**guard) };
        func(val)
    }

    /// Observe the value change of `T`.
    /// `read` is method is expected to be called immediately when true returns.
    pub fn has_changed(&mut self) -> bool {
        let tick_new = self.shared.tick.try_get().expect("Watched value is gone");
        if tick_new != self.tick {
            self.tick = tick_new;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[repr(C)]
    #[derive(Debug)]
    struct Foo([u8; 256]);

    #[test]
    fn works() {
        let mut mem = shared_memory_create("./test_file", 24).unwrap();

        let watched = Watched::<Foo>::new_from_mem(&mut mem);

        watched.write(Foo([123; 256]));

        std::thread::spawn(|| {
            let mut mem = shared_memory_open("./test_file", 24).unwrap();

            let mut watcher = Watcher::<Foo>::new_from_mem(&mut mem);

            assert!(watcher.has_changed());

            watcher.read(|foo| println!("foo is {:?}", foo));
        })
        .join()
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn drop_watched() {
        let mut mem = shared_memory_create("./test_file2", 24).unwrap();

        let watched = Watched::<Foo>::new_from_mem(&mut mem);

        drop(watched);

        let mut mem = shared_memory_open("./test_file2", 24).unwrap();

        let mut watcher = Watcher::<Foo>::new_from_mem(&mut mem);

        watcher.has_changed();
    }

    #[test]
    #[should_panic]
    fn size_check() {
        let mut mem = shared_memory_create("./test_file3", 20).unwrap();
        let watched = Watched::<Foo>::new_from_mem(&mut mem);
    }
}

#![feature(atomic_mut_ptr)]
#![feature(set_ptr_value)]

mod error;

use std::{
    pin::Pin,
    marker::{PhantomData, PhantomPinned}, mem, path::Path, sync::atomic::{AtomicUsize, Ordering}};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::AtomicU8;

use raw_sync::locks::{LockImpl, LockInit, RwLock};
use shared_memory::{ShmemConf, Shmem};

use crate::error::Error;

pub fn shared_memory_create(path: impl AsRef<Path>, size: usize) -> Result<Shmem, Error> {
    let mut mem = ShmemConf::new().size(size)
        .force_create_flink()
        .flink(path.as_ref()).create()?;
    assert!(mem.set_owner(true));
    Ok(mem)
}

pub fn shared_memory_open(path: impl AsRef<Path>, size: usize) -> Result<Shmem, Error> {
    let mut mem = ShmemConf::new().size(size).flink(path.as_ref()).open()?;
    Ok(mem)
}

pub struct Watched<'a, T> {
    tick: &'a mut AtomicU8,
    lock: Box<dyn LockImpl>,
    data: PhantomData<T>,
}

// The last bit of tick is used to mark the existence of active watcher
const TICK: u8 = 1 << 1;

impl<'a, T> Watched<'a, T> {
    pub fn new_in_shared(ptr: &mut Shmem) -> Self {

        let mut ptr = ptr.as_ptr();

        let tick: &'a mut AtomicU8;

        // SAFETY:
        // The actual size of it must be checked before adding to shared memory pointer.
        unsafe {
            let atomic_size = mem::size_of::<AtomicU8>();
            tick = &mut *(ptr as *mut u8 as *mut AtomicU8);
            ptr = ptr.add(atomic_size);
        }

        tick.store(0, Ordering::SeqCst);

        // SAFETY:
        // rwlock starts right after AtomicUsize's offset.
        let lock = unsafe { Self::rw_lock_new(ptr) };

        Watched {
            tick,
            lock,
            data: PhantomData,
        }
    }

    pub fn write(&self, value: T) {
        let mut guard = self.lock.lock().unwrap();

        // SAFETY:
        // This cast is safe. Watcher<T> type is the only type constructor expose.
        let val = unsafe { mem::transmute::<_, &mut T>(&mut **guard) };
        *val = value;

        self.tick.fetch_add(TICK, Ordering::SeqCst);
    }

    // SAFETY:
    // caller must make sure valid pointer is passed to RwLock is passed to constructor.
    // This includes the offset of RwLock itself and the data pointer the lock guard.
    unsafe fn rw_lock_new(ptr: *mut u8) -> Box<dyn LockImpl> {
        let (raw, _) = RwLock::new(ptr, ptr.add(RwLock::size_of(Some(ptr)))).unwrap();
        raw
    }
}

pub struct Watcher<'a, T> {
    tick: u8,
    shared_tick: &'a mut AtomicU8,
    lock: Box<dyn LockImpl>,
    data: PhantomData<T>,
}

impl<'a, T> Watcher<'a, T> {
    pub fn new_in_shared(ptr: &'a mut Shmem) -> Self {
        let mut ptr = ptr.as_ptr();

        let shared_tick: &'a mut AtomicU8;

        // SAFETY:
        // The actual size of it must be checked before adding to shared memory pointer.
        unsafe {
            let atomic_size = mem::size_of::<AtomicU8>();
            shared_tick = &mut *(ptr as *mut u8 as *mut AtomicU8);
            ptr = ptr.add(atomic_size);
        }

        // SAFETY:
        // rwlock starts right after AtomicUsize's offset.
        let lock = unsafe { Self::rw_lock_exist(ptr) };

        Watcher {
            tick: 0,
            shared_tick,
            lock,
            data: PhantomData
        }
    }

    pub fn read<F, O>(&self, func: F) -> O
        where
            F: FnOnce(&T) -> O,

    {
        let guard = self.lock.rlock().unwrap();
        let val = unsafe { mem::transmute::<_, &T>(&**guard) };
        func(val)
    }

    pub fn has_changed(&mut self) -> bool {
        let tick_new = self.shared_tick.load(Ordering::SeqCst);
        if tick_new != self.tick {
            self.tick = tick_new;
            true
        } else {
            false
        }
    }

    unsafe fn rw_lock_exist(ptr: *mut u8) -> Box<dyn LockImpl> {
        let (raw, _) = RwLock::from_existing(ptr, ptr.add(RwLock::size_of(Some(ptr)))).unwrap();
        raw
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[repr(C)]
    #[derive(Debug)]
    struct Foo([u8; 6]);

    #[test]
    fn works() {
        let mut mem = shared_memory_create("./test_file", 4096).unwrap();

        let watched = Watched::<Foo>::new_in_shared(&mut mem);

        watched.write(Foo([123; 6]));

        std::thread::spawn(|| {
            let mut mem = shared_memory_open("./test_file", 4096).unwrap();

            let mut watcher = Watcher::<Foo>::new_in_shared(&mut mem);

            assert!(watcher.has_changed());

            watcher.read(|foo| println!("foo is {:?}", foo));

        }).join().unwrap();
    }
}

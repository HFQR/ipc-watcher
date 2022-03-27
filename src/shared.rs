use std::{marker::PhantomData, mem};

use raw_sync::locks::{LockImpl, LockInit, RwLock};
use shared_memory::Shmem;

use crate::tick::Tick;

pub(crate) struct Shared<'a, T> {
    pub(crate) tick: Tick<'a>,
    pub(crate) lock: Box<dyn LockImpl>,
    data: PhantomData<T>,
}

impl<'a, T> Shared<'a, T> {
    pub(crate) fn new_from_mem(mem: &'a Shmem) -> Self {
        Self::from_mem(mem, |ptr, data_off| unsafe {
            // SAFETY:
            // Trust the pointer given by Shmem and data_off counted the size of RwLock.

            let (lock, _) = RwLock::new(ptr, ptr.add(data_off)).unwrap();
            lock
        })
    }

    pub(crate) fn exist_from_mem(mem: &'a Shmem) -> Self {
        Self::from_mem(mem, |ptr, data_off| unsafe {
            // SAFETY:
            // Trust the pointer given by Shmem and data_off counted the size of RwLock.
            let (lock, _) = RwLock::from_existing(ptr, ptr.add(data_off)).unwrap();
            lock
        })
    }

    // create Shared with a closure for rwlock constructing.
    fn from_mem<F>(mem: &'a Shmem, func: F) -> Self
    where
        F: FnOnce(*mut u8, usize) -> Box<dyn LockImpl>,
    {
        // Check for the size of shared memory.
        let shared_size = mem::size_of::<Self>();
        let mem_size = mem.len();
        assert!(
            shared_size <= mem_size,
            "Shared memory not enough, {} extra bytes needed",
            shared_size - mem_size
        );

        let mut ptr = mem.as_ptr();

        // SAFETY:
        // Shmem is borrowed for the same lifetime of Self so Tick's lifetime is satisfied.
        let tick = unsafe {
            let (tick, size) = Tick::from_ptr(ptr);
            ptr = ptr.add(size);
            tick
        };

        let data_off = RwLock::size_of(Some(ptr));

        let lock = func(ptr, data_off);

        Self {
            tick,
            lock,
            data: PhantomData,
        }
    }
}

use std::{
    mem,
    sync::atomic::{AtomicU8, Ordering},
};

// important. Tick must have the same layout of inner atomic counter.
#[repr(transparent)]
pub(crate) struct Tick<'a>(&'a mut AtomicU8);

// The last bit of tick is used to mark the existence of active watcher
const TICK: u8 = 1 << 1;

impl<'a> Tick<'a> {
    // SAFETY:
    // Caller must make sure given pointer is valid for the lifetime of Tick.
    pub(crate) unsafe fn from_ptr(ptr: *mut u8) -> (Self, usize) {
        let atomic_size = mem::size_of::<Self>();
        let tick = Tick(&mut *(ptr as *mut AtomicU8));

        (tick, atomic_size)
    }

    pub(crate) fn tick(&self) {
        self.0.fetch_add(TICK, Ordering::SeqCst);
    }

    pub(crate) fn store(&self, val: u8) {
        self.0.store(val, Ordering::SeqCst);
    }

    pub(crate) fn close(&self) {
        let val = self.try_get().unwrap();
        self.store(val | 1);
    }

    pub(crate) fn try_get(&self) -> Option<u8> {
        let val = self.0.load(Ordering::SeqCst);
        if val & 1 == 1 {
            None
        } else {
            Some(val)
        }
    }
}

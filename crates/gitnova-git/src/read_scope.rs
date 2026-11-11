//! Request-local read-only execution constraints. No process-global environment changes.
use gitnova_domain::{AppError, Result};
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};
#[derive(Clone)]
struct Scope {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
thread_local! { static ACTIVE: RefCell<Option<Scope>> = const { RefCell::new(None) }; }
pub fn with_read_scope<T>(
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    f: impl FnOnce() -> T,
) -> T {
    struct Restore(Option<Scope>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|s| *s.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(ACTIVE.with(|s| {
        s.replace(Some(Scope {
            deadline,
            cancelled,
        }))
    }));
    f()
}
pub(crate) fn active() -> bool {
    ACTIVE.with(|s| s.borrow().is_some())
}
pub(crate) fn check() -> Result<()> {
    ACTIVE.with(|s| {
        if let Some(s) = s.borrow().as_ref() {
            if s.cancelled.load(Ordering::Acquire) {
                return Err(AppError::new("cancelled", "Read cancelled."));
            }
            if Instant::now() >= s.deadline {
                return Err(AppError::new("timeout", "Read deadline exceeded."));
            }
        }
        Ok(())
    })
}

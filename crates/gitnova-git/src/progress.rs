//! Per-worker advisory progress, independent of the desktop transport.
use gitnova_domain::WorkProgress;
use std::cell::RefCell;
type Sink = Box<dyn Fn(WorkProgress)>;
thread_local! {static SINK: RefCell<Option<Sink>> = RefCell::new(None);}
struct Scope(Option<Sink>);
impl Drop for Scope {
    fn drop(&mut self) {
        SINK.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}
pub fn with_progress<T>(sink: impl Fn(WorkProgress) + 'static, run: impl FnOnce() -> T) -> T {
    let _scope = Scope(SINK.with(|slot| slot.replace(Some(Box::new(sink)))));
    run()
}
pub(crate) fn emit(phase: &str, completed: u64, total: u64, unit: &str) {
    SINK.with(|slot| {
        if let Some(sink) = slot.borrow().as_ref() {
            sink(WorkProgress {
                phase: phase.into(),
                completed,
                total,
                unit: unit.into(),
            });
        }
    });
}

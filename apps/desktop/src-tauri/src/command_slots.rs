//! Keep one of the existing two workers available to local foreground commands.
//! Waiting background reads do not acquire a worker. Never cancels mutations.
use std::sync::{Arc, LazyLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub struct CommandSlots {
    workers: Arc<Semaphore>,
    background: Arc<Semaphore>,
}
pub static SLOTS: LazyLock<CommandSlots> = LazyLock::new(CommandSlots::new);
impl CommandSlots {
    fn new() -> Self {
        Self {
            workers: Arc::new(Semaphore::new(2)),
            background: Arc::new(Semaphore::new(1)),
        }
    }
    pub async fn acquire(
        &self,
        background: bool,
    ) -> Result<(OwnedSemaphorePermit, Option<OwnedSemaphorePermit>), tokio::sync::AcquireError>
    {
        let reservation = if background {
            Some(self.background.clone().acquire_owned().await?)
        } else {
            None
        };
        Ok((self.workers.clone().acquire_owned().await?, reservation))
    }
}

#[cfg(test)]
mod tests {
    use super::CommandSlots;
    use std::{
        future::Future,
        pin::pin,
        task::{Context, Poll, Waker},
    };
    #[test]
    fn background_waiters_leave_a_foreground_slot_and_hold_permits_until_drop() {
        let slots = CommandSlots::new();
        let mut context = Context::from_waker(Waker::noop());
        let mut first = pin!(slots.acquire(true));
        let Poll::Ready(Ok(first)) = first.as_mut().poll(&mut context) else {
            panic!("first background read must start")
        };
        let mut second = pin!(slots.acquire(true));
        assert!(second.as_mut().poll(&mut context).is_pending());
        let mut foreground = pin!(slots.acquire(false));
        let Poll::Ready(Ok(foreground)) = foreground.as_mut().poll(&mut context) else {
            panic!("foreground slot must remain free")
        };
        assert!(slots.workers.clone().try_acquire_owned().is_err());
        drop(first);
        assert!(second.as_mut().poll(&mut context).is_ready());
        drop(foreground);
        assert_eq!(slots.workers.available_permits(), 2);
    }
}

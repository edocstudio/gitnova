//! Pipe completion wakes the owning child waiter immediately. Preserve ownership,
//! bounded deadlines and low polling cost when a child closes its pipes early.
use std::{
    io,
    process::{Child, ExitStatus},
    sync::mpsc::{Receiver, RecvTimeoutError},
    time::{Duration, Instant},
};
pub(crate) fn wait_for_exit(
    child: &mut Child,
    budget: Duration,
    wake: &Receiver<()>,
    tick: Duration,
) -> io::Result<Option<ExitStatus>> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        let elapsed = start.elapsed();
        if elapsed >= budget {
            return Ok(None);
        }
        let timeout = tick.min(budget.saturating_sub(elapsed));
        match wake.recv_timeout(timeout) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                std::thread::sleep(timeout.min(if elapsed < Duration::from_millis(30) {
                    Duration::from_millis(1)
                } else {
                    tick
                }))
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn preserves_deadline_and_reports_child_status() {
        use std::process::Command;
        let (tx, rx) = std::sync::mpsc::channel();
        let mut child = Command::new("/bin/sleep").arg("1").spawn().unwrap();
        assert!(wait_for_exit(
            &mut child,
            Duration::from_millis(15),
            &rx,
            Duration::from_millis(10)
        )
        .unwrap()
        .is_none());
        child.kill().unwrap();
        child.wait().unwrap();
        drop(tx);
        let mut child = Command::new("/usr/bin/true").spawn().unwrap();
        assert!(wait_for_exit(
            &mut child,
            Duration::from_secs(1),
            &rx,
            Duration::from_millis(10)
        )
        .unwrap()
        .unwrap()
        .success());
    }
}

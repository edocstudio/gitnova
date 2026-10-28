//! Opt-in, per-thread measurements. Records command names/counts only, never arguments,
//! paths, URLs, Git output or credentials. No logging or retained metrics by default.
use serde::Serialize;
use std::{cell::RefCell, collections::BTreeMap, time::Instant};
#[derive(Debug, Default, Serialize)]
pub struct GitMetrics {
    pub calls: u64,
    pub command_microseconds: u128,
    pub commands: BTreeMap<String, u64>,
}
thread_local! {static METRICS:RefCell<Option<GitMetrics>>=const {RefCell::new(None)};}
struct Scope(Option<GitMetrics>);
impl Drop for Scope {
    fn drop(&mut self) {
        METRICS.with(|m| {
            m.replace(self.0.take());
        });
    }
}
pub fn measure_git<T>(f: impl FnOnce() -> T) -> (T, GitMetrics) {
    let scope = Scope(METRICS.with(|m| m.replace(Some(GitMetrics::default()))));
    let result = f();
    let metrics = METRICS.with(|m| m.borrow_mut().take().unwrap_or_default());
    drop(scope);
    (result, metrics)
}
pub(crate) struct CommandTimer(Option<(Instant, String)>);
impl CommandTimer {
    pub fn new(command: &str) -> Self {
        Self(METRICS.with(|m| {
            m.borrow()
                .as_ref()
                .map(|_| (Instant::now(), command.chars().take(32).collect()))
        }))
    }
}
impl Drop for CommandTimer {
    fn drop(&mut self) {
        if let Some((start, name)) = self.0.take() {
            METRICS.with(|m| {
                if let Some(value) = m.borrow_mut().as_mut() {
                    value.calls += 1;
                    value.command_microseconds += start.elapsed().as_micros();
                    *value.commands.entry(name).or_default() += 1;
                }
            });
        }
    }
}

//! dessert: a tiny, generic discrete-event simulation (DES) framework
//!
//! Goals:
//! - Keep the core minimal and generic over user-defined `State` and `Event` types.
//! - Provide a classic event-scheduling loop with a priority queue (min-heap by time).
//! - Let events mutate simulation state and enqueue more events via a restricted `State` handle,
//!   while a separate `Engine` drives the main loop.
//!
//! Non-goals (for now): resources, processes, and distributions. These can be layered
//! on top later (e.g., a process/coroutine API that schedules future events).
//!
//! # Quick example
//!
//! ```
//! use dessert::{Engine, Event, State, Timestamp};
//!
//! #[derive(Default, Clone, Debug)]
//! struct Counter { pub ticks: u32 }
//!
//! #[derive(Clone, Debug)]
//! struct Tick { at: Timestamp, left: u32 }
//!
//! impl Event<Counter> for Tick {
//!     fn time(&self) -> Timestamp { self.at }
//!     fn execute(self, st: &mut State<Counter, Tick>) {
//!         st.state_mut().ticks += 1;
//!         if self.left > 0 {
//!             let next_at = self.at + 1.0; // 1 time-unit later
//!             st.schedule(Tick { at: next_at, left: self.left - 1 });
//!         }
//!     }
//! }
//!
//! let mut engine = Engine::<Counter, Tick>::new(Counter::default());
//! engine.schedule(Tick { at: 0.0, left: 4 });
//! engine.run_until(10.0);
//! assert_eq!(engine.state().ticks, 5);
//! ```

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::marker::PhantomData;

/// Simulation timestamp type (continuous time supported).
pub type Timestamp = f64;

/// Trait for events that mutate `State` and may schedule more events.
///
/// Implementors should be plain data types carrying the scheduled time and any payload
/// needed to execute. The engine calls `execute` when the event reaches the head of the
/// queue and the simulation time advances to its timestamp.
pub trait Event<S>: Sized {
    /// Time at which this event should fire.
    fn time(&self) -> Timestamp;

    /// Execute the event logic, mutating state and optionally scheduling more events
    /// via the provided state handle. Consumes the event (one-shot).
    fn execute(self, state: &mut State<S, Self>);
}

#[derive(Clone)]
struct Scheduled<S, E: Event<S>> {
    at: Timestamp,
    event: E,
    _marker: PhantomData<S>,
}

impl<S, E: Event<S>> Scheduled<S, E> {
    fn new(event: E) -> Self {
        let at = event.time();
        Self {
            at,
            event,
            _marker: PhantomData,
        }
    }
}

impl<S, E: Event<S>> PartialEq for Scheduled<S, E> {
    fn eq(&self, other: &Self) -> bool { self.at.total_cmp(&other.at) == Ordering::Equal }
}
impl<S, E: Event<S>> Eq for Scheduled<S, E> {}
impl<S, E: Event<S>> PartialOrd for Scheduled<S, E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
impl<S, E: Event<S>> Ord for Scheduled<S, E> {
    fn cmp(&self, other: &Self) -> Ordering { self.at.total_cmp(&other.at).reverse() }
}

/// The simulation state visible to events.
#[derive(Clone)]
pub struct State<S, E: Event<S>> {
    now: Timestamp,
    data: S,
    queue: BinaryHeap<Scheduled<S, E>>,
}

impl<S, E: Event<S>> State<S, E> {
    /// Create a new simulation state with user data.
    pub fn new(data: S) -> Self {
        Self {
            now: 0.0,
            data,
            queue: BinaryHeap::new(),
        }
    }

    /// Current simulation time.
    pub fn now(&self) -> Timestamp {
        self.now
    }

    /// Immutable access to user data.
    pub fn state(&self) -> &S {
        &self.data
    }

    /// Mutable access to user data.
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.data
    }

    /// Schedule an event at its own `Event::time()`.
    pub fn schedule(&mut self, event: E) {
        self.queue.push(Scheduled::new(event));
    }

}

/// The engine drives the event loop and owns the `State`.
pub struct Engine<S, E: Event<S>> {
    state: State<S, E>,
    /// Snapshots of the state after each executed event (and at start/end).
    history: Vec<State<S, E>>,
    /// Chronological event log: (time, label)
    events: Vec<(Timestamp, String)>,
}

impl<S: Clone, E: Event<S> + Clone + std::fmt::Debug> Engine<S, E> {
    /// Create a new engine with initial user state.
    pub fn new(data: S) -> Self {
        let state = State::<S, E>::new(data);
        let mut engine = Self { state, history: Vec::new(), events: Vec::new() };
        engine.history.push(engine.state.clone());
        engine
    }

    /// Accessors to read the state and time (outside of events).
    pub fn now(&self) -> Timestamp { self.state.now() }
    pub fn state(&self) -> &S { self.state.state() }
    pub fn state_mut(&mut self) -> &mut S { self.state.state_mut() }

    /// Allow external scheduling prior to running.
    pub fn schedule(&mut self, event: E) { self.state.schedule(event) }

    /// Run until the queue is empty or the time limit is reached.
    pub fn run_until(&mut self, until_time: Timestamp) {
        while let Some(scheduled) = self.state.queue.pop() {
            if scheduled.at > until_time {
                self.state.queue.push(scheduled);
                break;
            }
            self.state.now = scheduled.at;
            // Log the event before execution
            self.events.push((self.state.now, format!("{:?}", scheduled.event)));
            scheduled.event.execute(&mut self.state);
            self.history.push(self.state.clone());
        }
        if self.state.now < until_time { self.state.now = until_time; }
        if self.history.last().map(|s| s.now) != Some(self.state.now) {
            self.history.push(self.state.clone());
        }
    }

    /// Access the recorded state snapshots.
    pub fn history(&self) -> &[State<S, E>] { &self.history }

    /// Access the chronological event log.
    pub fn events(&self) -> &[(Timestamp, String)] { &self.events }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default, Clone)]
    struct Counter {
        ticks: u32,
    }

    #[derive(Clone, Debug)]
    struct Tick {
        at: Timestamp,
        left: u32,
    }

    impl Event<Counter> for Tick {
        fn time(&self) -> Timestamp {
            self.at
        }
        fn execute(self, state: &mut State<Counter, Tick>) {
            state.state_mut().ticks += 1;
            if self.left > 0 {
                let next_at = self.at + 0.5;
                state.schedule(Tick {
                    at: next_at,
                    left: self.left - 1,
                });
            }
        }
    }

    #[test]
    fn counter_advances() {
        let mut engine = Engine::<Counter, Tick>::new(Counter::default());
        engine.schedule(Tick { at: 0.0, left: 3 });
        engine.run_until(10.0);
        assert_eq!(engine.state().ticks, 4);
        assert!(engine.now() >= 10.0);
    }
}

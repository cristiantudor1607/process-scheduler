use crate::scheduler::Process;
use crate::common_types::Event;

pub trait ProcessManager {
    /// Enqueues the given process to the ready queue
    /// 
    /// * `proc` - process to be enqueued
    fn enqueue_process(&mut self, proc: &dyn Process);

    /// Dequeues the next process from ready queue
    fn dequeue_process(&mut self);

    /// Makes the process sleep for `time` units of time
    /// 
    /// * `proc` - process to be sent to sleep
    /// * `time` - units of time that process has to sleep
    fn sleep_process(&mut self, proc: &dyn Process, time: usize);

    /// Move back to `ready` state the processes that consumed their
    /// sleeping time
    fn awake_processes(&mut self);

    /// Blocks the processes until an event happen
    /// 
    /// * `proc` - process to be blocked
    /// * `event` - the event that process waits for
    fn block_process(&mut self, proc: &dyn Process, event: Event);

    /// Move back to `ready` state the processes that were waiting for
    /// the event to happen
    /// 
    /// * `event` - the event that processes were waiting for
    fn unblock_processes(&mut self, event: Event);
}
use crate::{ProcessState, Process, StopReason};
use crate::common_types::Event;

pub trait ProcessControlBlock: Process {
    fn inc_priority(&mut self);
    fn dec_priority(&mut self);

    fn set_state(&mut self, new_state: ProcessState);
    fn set_running(&mut self);
    fn set_sleeping(&mut self);
    fn wait_for_event(&mut self, e : Event);

    fn execute(&mut self, time: usize);
    fn syscall(&mut self);

    fn get_payload(&self) -> usize;
    fn load_payload(&mut self, payload: usize);

    fn get_interrupted(&mut self, remaining: usize, reason: StopReason) -> usize;
}

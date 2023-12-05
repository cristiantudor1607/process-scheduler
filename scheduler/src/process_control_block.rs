use crate::{Pid, ProcessState, Event, Timestamp, StopReason};

pub trait ProcessControlBlock {
    fn get_pid(&self) -> Pid;
    
    fn get_priority(&self) -> i8;
    fn inc_priority(&mut self);
    fn dec_priority(&mut self);

    fn get_state(&self) -> ProcessState;
    fn set_state(&mut self, state: ProcessState);
    fn set_running(&mut self);
    fn set_sleeping(&mut self);
    fn wait_for_event(&mut self, event: Event);

    fn get_arrival_time(&self) -> Timestamp;

    fn set_total_time(&mut self, time: usize);

    fn execute(&mut self, time: usize);
    fn syscall(&mut self);
    fn add_viruntime(&mut self, time: usize);

    fn get_payload(&self) -> usize;
    fn load_payload(&mut self, payload: usize);

    fn get_interrupted(&mut self, remaining: usize, reason: StopReason) -> usize;
}
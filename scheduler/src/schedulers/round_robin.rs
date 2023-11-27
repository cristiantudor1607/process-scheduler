use std::num::NonZeroUsize;
use crate::scheduler::{Pid, ProcessState, Process, SchedulingDecision, StopReason, 
SyscallResult, Scheduler};

pub struct RoundRobinPCB {
    pid: Pid,
    state: ProcessState,
    burst_time: usize,
    remaining_time: usize,
}

impl RoundRobinPCB {
    pub fn new(pid: Pid,
        state: ProcessState,
        burst_time: usize, 
        remaining_time: usize) 
        -> RoundRobinPCB {
        RoundRobinPCB {
            pid,
            state,
            burst_time,
            remaining_time,
        }
    }
}

impl Process for RoundRobinPCB {
    fn pid(&self) -> Pid {
        self.pid
    }

    fn state(&self) -> ProcessState {
        self.state
    }

    fn timings(&self) -> (usize, usize, usize) {
        (self.burst_time, 0, self.remaining_time)
    }

    fn priority(&self) -> i8 {
        0
    }

    fn extra(&self) -> String {
        String::new()
    }
}

pub struct RoundRobinScheduler {
    ready: Vec<Box<dyn Process>>,
    waiting: Vec<Box<dyn Process>>,
    time_quanta: NonZeroUsize,
    min_timeslice: usize
}

unsafe impl Send for RoundRobinScheduler {}


impl RoundRobinScheduler {
    pub fn new(time_quanta: NonZeroUsize, min_timeslice: usize) -> RoundRobinScheduler{
        let ready: Vec<Box<dyn Process>> = vec![];
        let waiting: Vec<Box<dyn Process>> = vec![];

        RoundRobinScheduler {
            ready,
            waiting,
            time_quanta,
            min_timeslice,
        }
    }
}

impl Scheduler for RoundRobinScheduler {
    fn next(&mut self) -> SchedulingDecision {
        SchedulingDecision::Sleep(NonZeroUsize::new(3).unwrap())
    }

    fn stop(&mut self, reason: StopReason) -> SyscallResult {
        SyscallResult::Success
    }

    fn list(&mut self) -> Vec<&dyn Process> {
        let mut result: Vec<&dyn Process> = Vec::new();
        result.extend(self.ready.iter().map(|pcb| pcb.as_ref()));
        result
    }
}
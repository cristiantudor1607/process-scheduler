use std::ops::Add;
use crate::{Pid, ProcessState, Timestamp, Event, Process, ProcessControlBlock};
use crate::Vruntime;


#[derive(Clone, Copy)]
pub struct FairPCB {
    pub pid: Pid,
    pub priority: i8,
    pub state: ProcessState,
    pub vruntime: Vruntime,
    pub arrival_time: Timestamp,
    pub total_time: usize,
    pub exec_time: usize,
    pub syscall_time: usize,
    pub time_payload: usize,
}

impl FairPCB {
    pub fn new(
        pid: Pid,
        priority: i8,
        arrival_time: Timestamp,
        vruntime: Vruntime)
        -> FairPCB {
        
        FairPCB {
            pid,
            priority,
            state: ProcessState::Ready,
            vruntime,
            arrival_time,
            total_time: 0,
            exec_time: 0,
            syscall_time: 0,
            time_payload: 0,
        }
    }
}

impl Process for FairPCB {
    fn pid(&self) -> Pid {
        self.pid
    }

    fn state(&self) -> ProcessState {
        self.state
    }

    fn timings(&self) -> (usize, usize, usize) {
        (self.total_time, self.syscall_time, self.exec_time)
    }

    fn priority(&self) -> i8 {
        self.priority
    }

    fn extra(&self) -> String {
        format!("vruntime={}", self.vruntime.get())
    }
}

impl ProcessControlBlock for FairPCB {
    fn inc_priority(&mut self) { }
    fn dec_priority(&mut self) { }

    fn set_state(&mut self, new_state: ProcessState) {
        self.state = new_state;
    }

    fn set_running(&mut self) {
        self.state = ProcessState::Running;
    }

    fn set_sleeping(&mut self) {
        self.state = ProcessState::Waiting { event: None };
    }

    fn wait_for_event(&mut self, e : Event) {
        self.state = ProcessState::Waiting { event: Some(e.get()) };
    }

    fn syscall(&mut self) {
        self.syscall_time += 1;
    }

    fn execute(&mut self, time: usize) {
        self.exec_time += time;
    }

    fn get_payload(&self) -> usize {
        self.time_payload
    }
    
    fn load_payload(&mut self, payload: usize) {
        self.time_payload = payload;
    }
}

impl PartialEq<Vruntime> for FairPCB {
    fn eq(&self, other: &Vruntime) -> bool {
        self.vruntime == *other
    }
}

impl PartialOrd<Vruntime> for FairPCB {
    fn partial_cmp(&self, other: &Vruntime) -> Option<std::cmp::Ordering> {
        self.vruntime.partial_cmp(other)
    }
}

impl Add<Vruntime> for FairPCB {
    type Output = Vruntime;

    fn add(self, rhs: Vruntime) -> Self::Output {
        self.vruntime.add(rhs.get())
    }
}

impl PartialEq<FairPCB> for FairPCB {
    fn eq(&self, other: &Self) -> bool {
        return if self.pid != other.pid {
            return false
        } else {
            true
        }
    }
}


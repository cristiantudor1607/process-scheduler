
use std::ops::Add;

use crate::{Pid, ProcessState, Timestamp, Vruntime, Process, ProcessControlBlock};
use crate::{Event, StopReason};

#[derive(Clone, Copy)]
pub struct GeneralProcess {
    pid: Pid,
    priority: i8,
    priority_at_born: i8,
    state: ProcessState,
    arrival_time: Timestamp,
    execution_time: usize,
    syscall_time: usize,
    total_time: usize,
    time_payload: usize,
    vruntime: Option<Vruntime>
}

impl GeneralProcess {
    pub fn new(pid: Pid, prio: i8, timestamp: Timestamp) -> Self {
        GeneralProcess {
            pid,
            priority: prio,
            priority_at_born: prio,
            state: ProcessState::Ready,
            arrival_time: timestamp,
            execution_time: 0,
            syscall_time: 0,
            total_time: 0,
            time_payload: 0,
            vruntime: None
        }
    }

    pub fn with_vruntime(pid: Pid, prio: i8, timestamp: Timestamp, vruntime: Vruntime) -> Self {
        GeneralProcess {
            pid,
            priority: prio,
            priority_at_born: prio,
            state: ProcessState::Ready,
            arrival_time: timestamp,
            execution_time: 0,
            syscall_time: 0,
            total_time: 0,
            time_payload: 0,
            vruntime: Some(vruntime),
        }
    }

}

impl Process for GeneralProcess {
    fn pid(&self) -> Pid {
        self.pid
    }

    fn state(&self) -> ProcessState {
        self.state
    }

    fn timings(&self) -> (usize, usize, usize) {
        (self.total_time, self.syscall_time, self.execution_time)
    }

    fn priority(&self) -> i8 {
        self.priority_at_born
    }

    fn extra(&self) -> String {
        return if let Some(vruntime) = self.vruntime {
            format!("vruntime={}", vruntime.get())
        } else {
            String::new()
        }
    }
}

impl ProcessControlBlock for GeneralProcess {
    fn get_pid(&self) -> Pid {
        self.pid
    }

    fn get_priority(&self) -> i8 {
        self.priority
    }

    fn inc_priority(&mut self) {
        self.priority += 1;
        if self.priority > self.priority_at_born {
            self.priority = self.priority_at_born;
        }
    }

    fn dec_priority(&mut self) {
        if self.priority == 0 {
            return;
        }

        self.priority -= 1;
    }

    fn get_state(&self) -> ProcessState {
        self.state
    }

    fn set_state(&mut self, state: ProcessState) {
        self.state = state;
    }

    fn set_running(&mut self) {
        self.set_state(ProcessState::Running);
    }

    fn set_sleeping(&mut self) {
        self.set_state(ProcessState::Waiting { event: None });
    }

    fn wait_for_event(&mut self, event: Event) {
        self.set_state(ProcessState::Waiting { event: Some(event.get()) });
    }

    fn get_arrival_time(&self) -> Timestamp {
        self.arrival_time
    }

    fn set_total_time(&mut self, time: usize) {
        self.total_time = time;
    }

    fn syscall(&mut self) {
        self.syscall_time += 1;
    }

    fn execute(&mut self, time: usize) {
        self.execution_time += time;
    }

    fn add_viruntime(&mut self, time: usize) {
        if self.vruntime.is_none() {
            return;
        }

        if let Some(mut runtime) = self.vruntime {
            runtime = runtime.add(time);
            self.vruntime = Some(runtime);
        }
    }

    fn get_payload(&self) -> usize {
        self.time_payload
    }

    fn load_payload(&mut self, payload: usize) {
        self.time_payload = payload;
    }

    fn get_interrupted(&mut self, remaining: usize, reason: crate::StopReason) -> usize {
        let exec_time: usize;

        if let StopReason::Syscall { .. } = reason {
            self.syscall();
            self.inc_priority();

            exec_time = self.get_payload() - remaining - 1;
            self.add_viruntime(exec_time + 1);
        } else {
            self.dec_priority();

            exec_time = self.get_payload() - remaining;
            self.add_viruntime(exec_time);
        }

        self.execute(exec_time);
        self.load_payload(remaining);

        exec_time
    }
}
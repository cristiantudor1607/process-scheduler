use crate::ProcessControlBlock;
use crate::scheduler::Process;

use crate::scheduler::Pid;
use crate::scheduler::ProcessState;
use crate::common_types::Timestamp;
use crate::common_types::Event;


/// The Priority Round Robin process control block
#[derive(Clone, Copy)]
pub struct PrioRoundRobinPCB {
    /// The PID of the process
    pub pid: Pid,
    /// The priority that process had when it was created
    /// 
    /// The priority of the process will never exceed this priority
    /// It cannot be greater than 5
    pub priority_at_born: i8,
    /// The priority of the process
    /// 
    /// The priority can't be smaller than 0, or greater than `priority_at_born`
    pub priority: i8,
    /// The state of the process
    pub state: ProcessState,
    /// The timestamp when process was spawned
    pub arrival_time: Timestamp,
    /// The total time of existence
    pub total_time: usize,
    /// The time that process spent executing instructions
    pub exec_time: usize,
    /// The time that process consumed on syscalls, or the
    /// number of syscalls that interrupted the process in all of
    /// his existence
    pub syscall_time: usize,
    /// The maximum time that process can execute instructions, before being
    /// preempted
    pub time_payload: usize,
}

unsafe impl Send for PrioRoundRobinPCB {}

impl PrioRoundRobinPCB {
    /// Creates a new Process Control Block
    /// 
    /// * `pid` - PID of the new process
    /// * `priority` - priority of the new process
    /// * `arrival` - timestamp when process is created
    pub fn new(pid: Pid, priority: i8, arrival: Timestamp) -> PrioRoundRobinPCB {
        PrioRoundRobinPCB {
            pid,
            priority,
            priority_at_born: priority,
            state: ProcessState::Ready,
            arrival_time: arrival,
            total_time: 0,
            exec_time: 0,
            syscall_time: 0,
            time_payload: 0,
        }
    }
}

impl Process for PrioRoundRobinPCB {
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
        String::new()
    }
}

impl ProcessControlBlock for PrioRoundRobinPCB {
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

    fn set_state(&mut self, new_state: ProcessState) {
        self.state = new_state;
    }

    fn set_running(&mut self) {
        self.set_state(ProcessState::Running);
    }

    fn set_sleeping(&mut self) {
        self.set_state(ProcessState::Waiting { event: None });
    }

    fn wait_for_event(&mut self, e : Event) {
        self.set_state(ProcessState::Waiting { event: Some(e.get()) });
    }

    fn execute(&mut self, time: usize) {
        self.exec_time += time;
    }

    fn syscall(&mut self) {
        self.syscall_time += 1;
    }

    fn get_payload(&self) -> usize {
        self.time_payload
    }

    fn load_payload(&mut self, payload: usize) {
        self.time_payload = payload;
    }
}
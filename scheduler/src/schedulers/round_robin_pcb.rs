use crate::{Pid, ProcessState, Process, ProcessControlBlock, Event, Timestamp};

/// The Round Robin Scheduler process control block
#[derive(Clone, Copy)]
pub struct RoundRobinPCB {
	/// The pid of the process
    pub pid: Pid,
    /// The priority of the process
    /// 
    /// The Round Robin Algorithm ignores the priorities
    pub priority: i8,
    /// The state of the process
	pub state: ProcessState,
    /// The time when process spawned
    pub arrival_time: Timestamp,
    /// The process time of existence
    pub total_time: usize,
    /// The time that process spent in execution
	pub exec_time: usize,
    /// The time that process spent on syscalls
	pub syscall_time: usize,
    /// The maximum time that the process can be in
    /// `running` state, when it is launched
    /// 
    /// The payload is set before running the process
    pub time_payload: usize,
}

impl RoundRobinPCB {
    /// Creates a new Process Control Block
    /// 
    /// * `pid` - pid of the new process
    /// * `priority` - priority of the new process
    /// * `arrival` - the timestamp when process was spawned
    pub fn new(pid: Pid, priority: i8, arrival: Timestamp) -> RoundRobinPCB {
        RoundRobinPCB {
            pid,
            priority,
            state: ProcessState::Ready,
            arrival_time: arrival,
            total_time: 0,
            exec_time: 0,
            syscall_time: 0,
            time_payload: 0,
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
		(self.total_time, self.syscall_time, self.exec_time)
	}

	fn priority(&self) -> i8 {
		self.priority
	}

	fn extra(&self) -> String {
		String::new()
	}
}

impl ProcessControlBlock for RoundRobinPCB {
    fn inc_priority(&mut self) {}
    fn dec_priority(&mut self) {}

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
        self.set_state(ProcessState::Waiting { event: Some(e.get()) })
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
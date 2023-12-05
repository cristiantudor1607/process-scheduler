use crate::{Pid, ProcessState, Process};

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
    pub arrival_time: usize,
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
    pub fn new(pid: Pid, priority: i8, arrival: usize) -> RoundRobinPCB {
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

    /// Set the Process Control Block state to `new_state`
    /// 
    /// * `new_state` - new state of the process
	pub fn set_state(&mut self, new_state: ProcessState) {
		self.state = new_state;
	}

    /// Set the process state to `Running`
    pub fn set_run(&mut self) {
        self.state = ProcessState::Running;
    }

    /// Set the process state to `Waiting(None)`, which indicates that a process
    /// is in sleep state
    pub fn set_sleep(&mut self) {
        self.state = ProcessState::Waiting { event: None };
    }

    /// Set the process state to `Waiting(event)`, which indicates the process is
    /// waiting for `event` to happen
    pub fn wait_for_event(&mut self, event: usize) {
        self.state = ProcessState::Waiting { event: Some(event) };
    }

    /// Add the `time` to the total time that the process spent executing
    /// 
    /// * `time` - time that process spent doing instructions
	pub fn execute(&mut self, time: usize) {
		self.exec_time += time;
	}

    /// Increase the time that the process spent sending syscalls
    /// 
	/// Counts a new syscall
	pub fn syscall(&mut self) {
		self.syscall_time += 1;
	}

    pub fn load_payload(&mut self, payload: usize) {
        self.time_payload = payload;
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
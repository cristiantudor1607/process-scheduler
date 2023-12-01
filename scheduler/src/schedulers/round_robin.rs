use std::{num::NonZeroUsize, collections::VecDeque, ops::Add};
use crate::{scheduler::{Pid, ProcessState, Process, SchedulingDecision, StopReason, 
SyscallResult, Scheduler}, Syscall};

/// The Round Robin Scheduler process control block
#[derive(Clone, Copy)]
pub struct RoundRobinPCB {
	/// The pid of the process
    pid: Pid,
    /// The state of the process
	state: ProcessState,
    /// The time when process spawned
    arrival_time: usize,
    /// The process time of existence
    total_time: usize,
    /// The time that process spent in execution
	exec_time: usize,
    /// The time that process spent on syscalls
	syscall_time: usize,
    /// The maximum time that the process can be in
    /// `running` state, when it is launched
    /// 
    /// The payload is set before running the process
    time_payload: usize,
    /// The time that process spent sleeping
	sleep_time: usize,
    /// The time that process spent waiting in the queue
    queued_time: usize,
    /// The time that process spent waiting for events
    waiting_time: usize,
    /// The number of syscalls that stopped the process in a time
    /// quanta, while being in state `running`
    syscalls_at_runtime: usize,
}

impl RoundRobinPCB {
    /// Creates a new Process Control Block based on the provided pid
    /// 
    /// * `pid` - pid of the new process
    /// * `arrival` - the timestamp when process was spawned
    fn new(pid: Pid, arrival: usize) -> RoundRobinPCB {
        RoundRobinPCB {
            pid,
            state: ProcessState::Ready,
            arrival_time: arrival,
            total_time: 0,
            exec_time: 0,
            syscall_time: 0,
            time_payload: 0,
            sleep_time: 0,
            queued_time: 0,
            waiting_time: 0,
            syscalls_at_runtime: 0,
        }
    }

    /// Set the Process Control Block state to `new_state`
    /// 
    /// * `new_state` - new state of the process
	fn set_state(&mut self, new_state: ProcessState) {
		self.state = new_state;
	}

    /// Set the process state to `running`
    fn run(&mut self) {
        self.state = ProcessState::Running;
    }

    /// Add the `time` to the total time that the process spent executing
    /// 
    /// * `time` - time that process spent doing instructions
	fn execute(&mut self, time: usize) {
		self.exec_time += time;
	}

	/// Add the `time` to the total time that the process spent sleeping
    /// 
    /// * `time` - time that process spent sleeping 
	fn sleep(&mut self, time: usize) {
		self.sleep_time += time;
	}

    /// Add the `time` to the total time that the process spent waiting for
    /// events
    /// 
    /// * `time` - time that process spent waiting
    fn wait_event(&mut self, time: usize) {
        self.waiting_time += time;
    }

    /// Add `time` to the total time that process spent waiting for other
    /// processes to be run
    /// 
    /// * `time` - time that process spent wainting in queue
    fn wait_in_queue(&mut self, time: usize) {
        self.queued_time += time;
    }

    /// Increase the time that the process spent sending syscalls
    /// 
	/// Counts a new syscall
	fn syscall(&mut self) {
		self.syscall_time += 1;
        self.syscalls_at_runtime += 1;
	}

    /// Resets the `syscalls_at_runtime` field of the process control block
    fn reset_runtime_syscalls(&mut self) {
        self.syscalls_at_runtime = 0;
    }

    fn load_payload(&mut self, payload: usize) {
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
		0
	}

	fn extra(&self) -> String {
		String::new()
	}
}

/// The structure that is used to control the scheduling of the processes
/// for the Round Robin Scheduler
pub struct RoundRobinScheduler {
    /// The queue for processes in `ready` state
    ready: VecDeque<RoundRobinPCB>,
    /// The vec that stores and manages processes in `sleep` state
    sleeping: Vec<(RoundRobinPCB, usize)>,
    /// The vec that stores and manages processes waiting for an event
    waiting: Vec<(RoundRobinPCB, usize)>,
    /// The running process
    running: Option<RoundRobinPCB>,
    /// The time quanta of the scheduler
    /// 
    /// The maximum time a process can spend in `running` state
    quanta: NonZeroUsize,
    /// The minimum remaining time that a process needs for being replanified just
    /// after a syscall
    /// 
    /// If the remaining time after syscall is greater or equal to the `min_time`, then
    /// the process will continue running, otherwise, it will be put in the `ready` queue,
    /// and it will be planified when it will be dequeued by the scheduler
    min_time: usize,
    /// The pid of the next process that will be spawned
    next_pid: Pid,
    /// The time that the last process didn't use from it's quanta, before being interrupted
    /// by a syscall
    unused_time: usize,
    /// Current time since process with PID 1 started
    timestamp: usize,
}

impl RoundRobinScheduler {
    pub fn new(timeslice: NonZeroUsize, minimum_remaining_timeslice: usize) -> RoundRobinScheduler {
		RoundRobinScheduler { 
			ready: VecDeque::new(),
            sleeping: Vec::new(),
            waiting: Vec::new(),
            running: None,
            quanta: timeslice,
            min_time: minimum_remaining_timeslice,
            next_pid: Pid::new(1),
            unused_time: 0,
            timestamp: 0,
		}
	}

    /// Updates the current timestamp of the scheduler
    /// 
    /// * `time` - the time passed between current timestamp update
    ///          and previous timestamp update
    fn update_timestamp(&mut self, time: usize) {
        self.timestamp += time;
    }

    /// Recomputes the total time for all processes from scheduler,
    /// based on the current timestamp and their arrival time
    fn update_total_time(&mut self) {

        // Update the running process
        if let Some(mut pcb) = self.running {
            pcb.total_time = self.timestamp - pcb.arrival_time - 1;
            self.running = Some(pcb);
        }

        // Update the processes from ready queue
        for item in self.ready.iter_mut() {
            item.total_time = self.timestamp - item.arrival_time - 1;
        }

        // Update the processes from sleeping vec
        for item in self.sleeping.iter_mut() {
            item.0.total_time = self.timestamp - item.0.arrival_time - 1;
        }

        // Update the processes from waiting vec
        for item in self.waiting.iter_mut() {
            item.0.total_time = self.timestamp - item.0.arrival_time - 1;
        }
    }

    /// Increase the `next_pid` field of the RoundRobinScheduler
	/// 
	/// Method is called immediatelly after the `next_pid` is used by
    /// a process
	/// 
	/// The `next_pid` field is strongly connected with the creation of 
	/// processes, so the actions should happen one after another
	fn inc_pid(&mut self) {
		self.next_pid = self.next_pid.add(1);
	}

    /// Creates a new Process Control Block related to a new process, that uses the
    /// `next_pid` as pid
    ///
    /// Function calls [`inc_pid`] to update `next_pid`, after creating the process
    /// control block
    /// 
    /// * `timestamp` - the arrival time of the forked process
	fn spawn_process(&mut self, timestamp: usize) -> RoundRobinPCB {
		let new_proc = RoundRobinPCB::new(self.next_pid, timestamp);
		self.inc_pid();

		new_proc
	}

    /// Returns a new Round Robin process block, forked from the running process
    /// and pushes it to the `ready` queue
    fn fork(&mut self, timestamp: usize) -> RoundRobinPCB {
        /* Spawn a new process and add it to the `ready` queue */
        let new_proc = self.spawn_process(timestamp);
        self.ready.push_back(new_proc);

        new_proc
    }

    /// It kills the running process and returns Success, if it exists, otherwise
    /// it does nothing and returns NoRunningProcess
    fn kill_running_process(&mut self) -> SyscallResult {
        return if let Some(_) = self.running {
            self.running = None;
            SyscallResult::Success
        } else {
            SyscallResult::NoRunningProcess
        }
    }

    /// Enqueues the process sent as parameter to the `ready` queue
    /// 
    /// * `proc` - process to be enqueued
    fn enqueue_process(&mut self, mut proc: RoundRobinPCB) {
        proc.set_state(ProcessState::Ready);
        self.ready.push_back(proc);
    }

    /// Dequeues the next ready process in the queue, if there is one
    /// 
    /// The method does not modify the `ready` state of the process
    fn dequeue_ready_process(&mut self) {
        self.running = self.ready.pop_front();
    }

}

impl Scheduler for RoundRobinScheduler {
    fn stop(&mut self, reason: StopReason) -> SyscallResult {
        // Process stopped by a system call
        if let StopReason::Syscall { syscall, remaining } = reason {

            if let Syscall::Fork(_) = syscall {
                let new_proc: RoundRobinPCB;

                if let Some(mut pcb) = self.running {
                    // Fork from a parent process
                    
                    // Count the syscall and modify the payload
                    pcb.syscall();
                    
                    // Add the execution time
                    pcb.execute(pcb.time_payload - remaining - 1);

                    // Update the timestamp: -1 comes from the current syscall
                    self.update_timestamp(pcb.time_payload - remaining - 1);

                    // Change the payload
                    pcb.load_payload(remaining);

                    self.running = Some(pcb);

                    // Spawn the new process
                    new_proc = self.fork(self.timestamp);
                    self.update_timestamp(1);
                } else {
                    // The Fork that creates the process with PID 1 at timestamp 0

                    // Spawn the process with PID 1 at time 0
                    new_proc = self.fork(0);

                    // The operations consumes 1 unit of time
                    self.update_timestamp(1);
                }

                return SyscallResult::Pid(new_proc.pid);
            }

            if let Syscall::Exit = syscall {
                if let Some(pcb) = self.running {
                    self.update_timestamp(pcb.time_payload - remaining);
                }
                
                return self.kill_running_process();
            }
        }

        // If the process wasn't interrupted by a syscall, it's time expired
        if let Some(mut pcb) = self.running {
            // Count the execution time
            pcb.execute(pcb.time_payload);

            // Update the timestamp
            self.update_timestamp(pcb.time_payload);
            
            // Reset the payload, because the process will be put in `ready` state
            pcb.time_payload = 0;

            // Enqueue the running process with fields updated
            self.enqueue_process(pcb);
            self.running = None;

            return SyscallResult::Success;
        }

        SyscallResult::NoRunningProcess
    }

    fn next(&mut self) -> SchedulingDecision {
        
        // If the last running process expired, then running is None
        if let None = self.running {
            self.dequeue_ready_process();

            // If there was at least one process in the ready queue 
            if let Some(mut pcb) = self.running {
                // Run the process
                pcb.run();
                pcb.load_payload(self.quanta.get());
                self.running = Some(pcb);
                
                return SchedulingDecision::Run {
                    // It's safe to use unwrap
                    // TODO: add explanation
                    pid: self.running.unwrap().pid,
                    timeslice: NonZeroUsize::new(self.running.unwrap().time_payload).unwrap(),
                };
            }

            // At this point the ready queue  was empty before trying to dequeue a process
            // TODO: Check for a deadlock
            return SchedulingDecision::Done;
        }

        if let Some(proc) = self.running {
            // Case 1: It cannot continue running
            if proc.time_payload < self.min_time {
                self.enqueue_process(proc);

                self.dequeue_ready_process();

                if let Some(mut ready_proc) = self.running {
                    ready_proc.run();
                    ready_proc.load_payload(self.quanta.get());
                    self.running = Some(ready_proc);
                }

                // Because it enqueues and then dequeues from the same queue,
                // it's safe to use unwrap
                return SchedulingDecision::Run {
                    pid: self.running.unwrap().pid,
                    timeslice: self.quanta,
                };
            }

            // Case 2: The process will continue running
            return SchedulingDecision::Run {
                pid: proc.pid,
                // TODO: explain why this is alright
                timeslice: NonZeroUsize::new(proc.time_payload).unwrap(),
            };
        }

        panic!("Fatal error");

    }

    fn list(&mut self) -> Vec<&dyn Process> {
       
        self.update_total_time();
        let mut procs: Vec<&dyn Process> = Vec::new();

        /* Add the running process */
        match &self.running {
            Some(pcb) => procs.push(pcb),
            None => (),
        }

        /* Add the ready processes */
        for item in self.ready.iter() {
            procs.push(item);
        }

        /* Add the sleeping processes */
        for item in self.sleeping.iter() {
            procs.push(&item.0);
        }

        /* Add the waiting processes */
        for item in self.waiting.iter() {
            procs.push(&item.0);
        }

        procs
    }
}
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
    /// The time that process spent in execution
	exec_time: usize,
    /// The time that process spent on syscalls
	syscall_time: usize,
    /// The time that process spent sleeping
	sleep_time: usize,
    /// The number of syscalls that stopped the process in a time
    /// quanta, while being in state `running`
    syscalls_at_runtime: usize,
}

impl RoundRobinPCB {
    /// Creates a new Process Control Block based on the provided pid
    /// 
    /// * `pid` - pid of the new process
    fn new(pid: Pid) -> RoundRobinPCB {
        RoundRobinPCB {
            pid,
            state: ProcessState::Ready,
            exec_time: 0,
            syscall_time: 0,
            sleep_time: 0,
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

}

impl Process for RoundRobinPCB {
    fn pid(&self) -> Pid {
		self.pid
	}

	fn state(&self) -> ProcessState {
		self.state
	}

	fn timings(&self) -> (usize, usize, usize) {
		let total = self.exec_time + self.syscall_time + self.sleep_time;
		
		(total, self.syscall_time, self.exec_time)
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
    /// Function calls [`inc_pid`] to update `next_pid`, after creating the **PCB**
	fn spawn_process(&mut self) -> RoundRobinPCB {
		let new_proc = RoundRobinPCB::new(self.next_pid);
		self.inc_pid();

		new_proc
	}

    /// Returns a new Round Robin process block, forked from the running process
    /// and pushes it to the `ready` queue
    fn fork(&mut self) -> RoundRobinPCB {
        /* Spawn a new process and add it to the `ready` queue */
        let new_proc = self.spawn_process();
        self.ready.push_back(new_proc);

        new_proc
    }

    /// Enqueues the running process and returns Ok, if there is one,
    /// or it does nothing and returns an Err
    fn enqueue_running_process(&mut self) -> Result<(), ()>{
        return if let Some(mut pcb) = self.running {
            pcb.set_state(ProcessState::Ready);
            self.ready.push_back(pcb);
            self.running = None;

            // Let the caller know that the method made some changes
            Ok(())
        } else {

            // Let the caller know that there was no running process
            Err(())
        }
    }

    /// Dequeues the next ready process in the queue, if there is one
    /// 
    /// The method does not modify the `ready` state of the process
    fn dequeue_ready_process(&mut self) {
        self.running = self.ready.pop_front();
    }

    /// Destroys the running process and returns Ok, if there is a process
    /// running, otherwise returns an Err
    fn destroy_running_process(&mut self) -> Result<(), ()>{
        return if let Some(_) = self.running {
            self.running = None;

            Ok(())
        } else {
            Err(())
        }
    }
}

impl Scheduler for RoundRobinScheduler {
    fn stop(&mut self, reason: StopReason) -> SyscallResult {
        
        if let StopReason::Syscall { syscall, remaining } = reason {
            // Fork should be called with a process running, but the first fork, that
            // generates the process with PID 1 is called without having a process in
            // `running` state
            if let Syscall::Fork(_) = syscall {
                let new_proc: RoundRobinPCB;

                if let Some(mut pcb) = self.running {
                    // Count the syscalls
                    pcb.syscall();
                    self.running = Some(pcb);

                    // Save the timing for next scheduling decision
                    self.unused_time = remaining;
                    // Spawn new process
                    new_proc = self.fork();
                } else {
                    // Just spawn a new process
                    new_proc = self.fork();
                }

                return SyscallResult::Pid(new_proc.pid);
            }

            if let Syscall::Exit = syscall {
                if let Ok(()) = self.destroy_running_process() {
                    return SyscallResult::Success;
                } else {
                    return SyscallResult::NoRunningProcess;
                }
            }
        }

        // If the process wasn't interrupted by a syscall, then it's time expired
        if let Some(mut pcb) = self.running {
            // Set the unused_time
            self.unused_time = 0;
            
            // Calculate and add the execution time
            let exec_time = self.quanta.get() - pcb.syscalls_at_runtime;
            pcb.execute(exec_time);

            // Reset the runtime syscalls
            pcb.reset_runtime_syscalls();

            self.running = Some(pcb);

            // Enqueue the running process. It will always return Ok, 'cause
            // it can't reach this point if there's no process running
            _ = self.enqueue_running_process();
            return SyscallResult::Success;
        }

        SyscallResult::NoRunningProcess
    }

    fn next(&mut self) -> SchedulingDecision {
        
        let used_time = self.quanta.get() - self.unused_time;

        // If unused_time equals 0, either the program started and process 1 was forked,
        // or the last process that was running consumed all it's quanta
        if self.unused_time == 0 {
            self.dequeue_ready_process();

            if let Some(mut pcb) = self.running {
                // Run the process
                pcb.run();
                self.running = Some(pcb);

                return SchedulingDecision::Run {
                    pid: pcb.pid,
                    timeslice: self.quanta };
            } else {
                
                // TODO: Is not done, but for test 1 it should work
                return SchedulingDecision::Done;
            }
        }

        // If the running process can't be replanified
        if self.unused_time < self.min_time {
            
            // Reset the `unused_time`
            self.unused_time = 0;

            // It will always enter here, because the `unused_time` cannot be
            // > 0 if there isn't a process running
            if let Some(mut pcb) = self.running {
                let exec_time = used_time - pcb.syscalls_at_runtime;
                pcb.execute(exec_time);
                pcb.reset_runtime_syscalls();
                self.running = Some(pcb);
            }

            // Enqueue the running process. In our case, it will always return Ok,
            // because the `unused_time` is 0 if there isn't a process running at the
            // time next method is called
            self.enqueue_running_process().unwrap();

            // Dequeue the next process
            self.dequeue_ready_process();
            
            // The running can't be run, because it's a cycle
            // TODO: add unwrap here
            if let Some(mut pcb) = self.running {
                pcb.run();
                self.running = Some(pcb);

                return SchedulingDecision::Run {
                    pid: pcb.pid,
                    timeslice: self.quanta,
                }
            } else {
                panic!("I just enqueued a process");
            }
        }

        // At this point the running process should be replanified
        
        // Save the value of `unused_time`, to use it as timeslice to continue
        // running the current process
        let time: NonZeroUsize = NonZeroUsize::new(self.unused_time).unwrap();

        // Reset the `unused_time`
        self.unused_time = 0;
        
        return SchedulingDecision::Run {
            pid: self.running.unwrap().pid,
            timeslice: time,
        };

    }

    fn list(&mut self) -> Vec<&dyn Process> {
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
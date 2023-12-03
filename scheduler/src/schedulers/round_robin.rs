use std::{num::NonZeroUsize, collections::VecDeque, ops::Add};
use crate::{scheduler::{Pid, ProcessState, Process, SchedulingDecision, StopReason, 
SyscallResult, Scheduler}, Syscall};

#[derive(Debug)]
/// The Round Robin Scheduler process control block
#[derive(Clone, Copy)]
pub struct RoundRobinPCB {
	/// The pid of the process
    pid: Pid,
    /// The priority of the process
    /// 
    /// The Round Robin Algorithm ignores the priorities
    priority: i8,
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
}

impl RoundRobinPCB {
    /// Creates a new Process Control Block based on the provided pid
    /// 
    /// * `pid` - pid of the new process
    /// * `priority` - priority of the new process
    /// * `arrival` - the timestamp when process was spawned
    fn new(pid: Pid, priority: i8, arrival: usize) -> RoundRobinPCB {
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
	fn set_state(&mut self, new_state: ProcessState) {
		self.state = new_state;
	}

    /// Set the process state to `Running`
    fn set_run(&mut self) {
        self.state = ProcessState::Running;
    }

    /// Set the process state to `Waiting(None)`, which indicates that a process
    /// is in sleep state
    fn set_sleep(&mut self) {
        self.state = ProcessState::Waiting { event: None };
    }

    /// Set the process state to `Waiting(event)`, which indicates the process is
    /// waiting for `event` to happen
    fn wait_for_event(&mut self, event: usize) {
        self.state = ProcessState::Waiting { event: Some(event) };
    }

    /// Add the `time` to the total time that the process spent executing
    /// 
    /// * `time` - time that process spent doing instructions
	fn execute(&mut self, time: usize) {
		self.exec_time += time;
	}

    /// Increase the time that the process spent sending syscalls
    /// 
	/// Counts a new syscall
	fn syscall(&mut self) {
		self.syscall_time += 1;
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
		self.priority
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
    /// 
    /// It stores tuples (`Process Control Block`, Timestamp, Time), where
    /// * `Process Control Block` - stores the process data
    /// * `Timestamp` - the process was sent to sleep at this timestamp
    /// * `Time` - the process has to sleep this amount of time
    sleeping: Vec<(RoundRobinPCB, usize, usize)>,
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
    /// Current time since process with PID 1 started
    timestamp: usize,
    /// If the process with PID 1 is killed, panicd is activated
    panicd: bool,
    /// The time that scheduler has slept
    slept_time: usize,
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
            timestamp: 0,
            panicd: false,
            slept_time: 0,
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
    /// * `priority` - the priority of the new process
    /// * `timestamp` - the arrival time of the forked process
	fn spawn_process(&mut self, priority: i8, timestamp: usize) -> RoundRobinPCB {
		let new_proc = RoundRobinPCB::new(self.next_pid, priority, timestamp);
		self.inc_pid();

		new_proc
	}

    /// Returns a new Round Robin process block, forked from the running process
    /// and pushes it to the `ready` queue
    /// 
    /// * `priority` - the priority of the new process
    /// * `timestamp` - the time when process is created
    fn fork(&mut self, priority: i8, timestamp: usize) -> RoundRobinPCB {
        /* Spawn a new process and add it to the `ready` queue */
        let new_proc = self.spawn_process(priority, timestamp);
        self.ready.push_back(new_proc);

        new_proc
    }

    /// It kills the running process and returns Success, if it exists, otherwise
    /// it does nothing and returns NoRunningProcess
    fn kill_running_process(&mut self) -> SyscallResult {
        return if let Some(proc) = self.running {
            // Warn about a potential panic
            if proc.pid == Pid::new(1) {
                self.panicd = true;
            }

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

    // TODO: this don't need to be a method, it can be a function

    /// When a process is interrupted, this method updates the timings
    /// of the process and returns the time passed between last
    /// scheduler interference and the current time
    /// 
    /// * `running` - running process that is being interrupted
    /// * `remaining` - number of units of time that the running process
    ///               didn't use from it's quanta
    /// * `reason` - the reason why process stopped
    fn interrupt_process(&self,
        running: &mut RoundRobinPCB,
        remaining: usize,
        reason: StopReason)
        -> usize {
        
        let exec_time: usize;

        // Count a new syscall if the process was stopped by one
        if let StopReason::Syscall { .. } = reason {
            running.syscall();
            // For processes stopped by syscalls, one unit of time is used for the
            // syscall, not executed
            exec_time = running.time_payload - remaining - 1;
        } else {
            exec_time = running.time_payload - remaining;
        }

        // Count the execution time
        running.execute(exec_time);

        // Update the payload
        running.load_payload(remaining);

        return exec_time;
    }

    /// Adds the process to the sleeping queue of the scheduler
    /// 
    /// * `running` - the process to be put in the sleeping queue;
    ///             it should be the running process of the scheduler
    /// * `time` -  the time that process has to sleep
    fn sleep_process(&mut self, mut running: RoundRobinPCB, time: usize) {
        running.set_sleep();
        self.sleeping.push((running, self.timestamp, time));
    }

    /// Adds a process to the waiting queue of the scheduler
    /// 
    /// * `running` - the process to be put in the waiting queue;
    ///             if `running` isn't the scheduler's running process,
    ///             it will activate an undefined behaviour
    /// * `event` - the event that process is waiting for
    fn block_process(&mut self, mut running: RoundRobinPCB, event: usize) {
        running.wait_for_event(event);
        self.waiting.push((running, event));
    }

    /// Removes from waiting queue the processes that waited for the `event`
    /// to happen
    /// 
    /// * `event` - the event provided by a `Signal(event)` syscall
    fn unblock_processes(&mut self, event: usize) {
        let mut procs: VecDeque<RoundRobinPCB> = VecDeque::new();

        // Keep the processes that waits for another event
        self.waiting.retain(|item| {
            if item.1 == event {
                procs.push_back(item.0);
                false
            } else {
                true
            }
        });

        for item in procs.iter() {
            self.enqueue_process(*item);
        }
    }

    /// Updates the sleeping time of all the processes from sleeping queue
    fn update_sleeping_times(&mut self) {
        
        let curr_time = self.timestamp;

        for item in self.sleeping.iter_mut() {
            let time_diff = curr_time - item.1;
            if time_diff > item.2 {
                item.2 = 0;
            }
        }   
    }

    /// Sends to `ready` queue the processes that finised their sleep time
    fn wakeup_processes(&mut self) {
        let mut procs : VecDeque<RoundRobinPCB> = VecDeque::new();
        
        // Keep in sleeping queue the processes that didn't consumed it's
        // sleeping time
        self.sleeping.retain(| item | {
            if item.2 == 0 {
                procs.push_back(item.0);
                false
            } else {
                true
            }
        });

        // Enqueue the processes that slept all time
        for item in procs.iter() {
            self.enqueue_process(*item);
        }
    }

    /// Returns the minimum time that the scheduler has to sleep, so a process can
    /// be woken up
    fn get_sleep_time(&self) -> usize {
        if self.sleeping.is_empty() {
            return 0;
        }

        let mut min_time = usize::MAX;

        for item in self.sleeping.iter() {
            let ending_timestamp = item.1 + item.2;
            let remaining = ending_timestamp - self.timestamp + 1;
            if remaining < min_time {
                min_time = remaining;
            }
        }

        return min_time;
    }

    /// Checks if the parent process was killed, and his children still exist,
    /// a case of panic
    fn is_panicd(&self) -> bool {
        if !self.panicd {
            return  false;
        }

        if !self.ready.is_empty() || !self.sleeping.is_empty() || !self.waiting.is_empty() {
            return true;
        }

        if let Some(_) = self.running {
            return true;
        }

        return false;
    }

    /// Checks if the scheduler has to sleep, is deadlocked, or all the processes are done,
    /// and returns a SchedulingDecision, otherwise returns None
    fn is_blocked(&self) -> Option<SchedulingDecision> {
        // If there is a running process, the is_blocked functon should not be called,
        // based on the flow and logic of the program, but as a measure of safety, it'll
        // return None
        if let Some(_) = self.running {
            return None;
        }

        // If the ready queue isn't empty, the scheduler will continue planifying other
        // processes
        if !self.ready.is_empty() {
            return None;
        }

        // If all the queues are empty, then it is done
        if self.sleeping.is_empty() && self.waiting.is_empty() {
            return Some(SchedulingDecision::Done);
        }

        // If all the processes from the scheduler wait for an event to happen,
        // but there is no process that will send a signal, results a Deadlock
        if self.sleeping.is_empty() && !self.waiting.is_empty() {
            return Some(SchedulingDecision::Deadlock);
        }

        // If there are processes sleeping (no matter the waiting ones), the scheduler
        // should sleep
        if !self.sleeping.is_empty() {
            // TODO: explain why this is not 0
            let sleeping_time = self.get_sleep_time();
            
            return Some(SchedulingDecision::Sleep(NonZeroUsize::new(sleeping_time).unwrap()));
        }

        return None;
    }
    
    /// Decides if the scheduler has to sleep, by setting the `slept_time` field of the
    /// scheduler, based on the `decision` provided
    /// 
    /// * `decision` - the scheduling decision taken by the scheduler
    fn decide_sleep(&mut self, decision: SchedulingDecision) {
        if let SchedulingDecision::Sleep(time) = decision {
            self.slept_time = time.get();
        } else {
            self.slept_time = 0;
        }
    }

    /// If the scheduler slept, wakes it up, by setting the `timestamp` and reseting the
    /// `slept_time`. It also wakes up the processes that finised their sleeping
    fn wakeup_myself(&mut self) {
        if self.slept_time != 0 {
            self.update_timestamp(self.slept_time);
            self.slept_time = 0;
        }

        // TODO: add comment that it uses those 2
        self.update_sleeping_times();
        self.wakeup_processes();
    }
}

impl Scheduler for RoundRobinScheduler {
    fn stop(&mut self, reason: StopReason) -> SyscallResult {
        // Process stopped by a system call
        if let StopReason::Syscall { syscall, remaining } = reason {

            if let Syscall::Fork(prio) = syscall {
                let new_proc: RoundRobinPCB;

                if let Some(mut pcb) = self.running {
                    // Fork from a parent process

                    // Update the timings
                    let passsed_time = self.interrupt_process(&mut pcb, remaining, reason);
                    self.running = Some(pcb);
                    
                    // Update the timestamp
                    self.update_timestamp(passsed_time);

                    // Spawn the new process
                    new_proc = self.fork(prio, self.timestamp);
                    self.update_timestamp(1);   // The fork consumes one unit of time
                } else {
                    // The Fork that creates the process with PID 1 at timestamp 0

                    // Spawn the process with PID 1 at time 0
                    new_proc = self.fork(prio, 0);

                    self.update_timestamp(1); // The fork consumes one unit of time
                }

                return SyscallResult::Pid(new_proc.pid);
            }

            if let Syscall::Sleep(time) = syscall {
                if let Some(mut pcb) = self.running {   
                    // Update the timings anf the timestamp
                    let passed_time = self.interrupt_process(&mut pcb, remaining, reason);
                    self.update_timestamp(passed_time);

                    // Sleep the process
                    self.sleep_process(pcb, time);
                    self.update_timestamp(1); // The syscall consumes one unit of time
                    
                    // Set running to None
                    self.running = None;
                    return SyscallResult::Success;
                }
            }

            if let Syscall::Wait(event) = syscall {
                if let Some(mut pcb) = self.running {
                    // Update the timings and the timestamp
                    let passed_time = self.interrupt_process(&mut pcb, remaining, reason);
                    self.update_timestamp(passed_time);

                    // Make the process wait for event
                    self.block_process(pcb, event);
                    self.update_timestamp(1); // The syscall consumes one unit of time

                    self.running = None;
                    return SyscallResult::Success;
                }
            }

            if let Syscall::Signal(event) = syscall {
                if let Some(mut pcb) = self.running {
                    // Update the timings and the timestamp
                    let passed_time = self.interrupt_process(&mut pcb, remaining, reason);
                    self.update_timestamp(passed_time);

                    // Unblock all processes waiting for the event to happen
                    self.unblock_processes(event);
                    self.update_timestamp(1); // The syscall consumes one unit of time
                    self.running = Some(pcb);

                    return SyscallResult::Success;
                }
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
            // Update the timings
            let passed_time = self.interrupt_process(&mut pcb, 0, StopReason::Expired);

            // Update the timestamp
            self.update_timestamp(passed_time);

            // TODO: remove this
            self.wakeup_myself();
            
            // Enqueue the running process with fields updated
            self.enqueue_process(pcb);
            self.running = None;

            return SyscallResult::Success;
        }

        SyscallResult::NoRunningProcess
    }

    fn next(&mut self) -> SchedulingDecision {
        // Wake up the scheduler
        self.wakeup_myself();

        // If the last running process expired, then running is None
        if let None = self.running {
            
            // If there is no running process, at least one of the queues aren't empty,
            // and the parent process (PID 1) was killed, then there is a panic
            if self.is_panicd()  {
                return SchedulingDecision::Panic;
            }
            
            self.dequeue_ready_process();

            // If there was at least one process in the ready queue 
            if let Some(mut pcb) = self.running {
                // Run the process
                pcb.set_run();
                pcb.load_payload(self.quanta.get());
                self.running = Some(pcb);
                
                return SchedulingDecision::Run {
                    // It's safe to use unwrap
                    // TODO: add explanation
                    pid: self.running.unwrap().pid,
                    timeslice: NonZeroUsize::new(self.running.unwrap().time_payload).unwrap(),
                };
            }

            if let Some(result) = self.is_blocked() {
                self.decide_sleep(result);
                return result;
            }

            // It shouldn't reach this point
            panic!("Fatal error");
        }

        if let Some(proc) = self.running {
            // Case 1: It cannot continue running
            if proc.time_payload < self.min_time {
                self.enqueue_process(proc);

                self.dequeue_ready_process();

                if let Some(mut ready_proc) = self.running {
                    ready_proc.set_run();
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
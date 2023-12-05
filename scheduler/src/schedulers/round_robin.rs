use std::{num::NonZeroUsize, collections::VecDeque, ops::Add};
use crate::scheduler::{Pid, ProcessState, Process, SchedulingDecision, StopReason, 
SyscallResult, Scheduler};
use crate::Syscall;
use crate::{Collector, ProcessControlBlock, collect_all};
use crate::{Event, Timestamp};
use crate::RoundRobinPCB;


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
    sleeping: Vec<(RoundRobinPCB, Timestamp, usize)>,
    /// The vec that stores and manages processes waiting for an event
    waiting: Vec<(RoundRobinPCB, Event)>,
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
    timestamp: Timestamp,
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
            timestamp: Timestamp::new(0),
            panicd: false,
            slept_time: 0,
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

    /// Updates the current timestamp of the scheduler
    /// 
    /// * `time` - the time passed from the last intervention of the scheduler
    fn make_timeskip(&mut self, time: usize) {
        self.timestamp = self.timestamp.add(time);
    }

    /// Recomputes the total time for all processes from scheduler,
    /// based on the current timestamp and their arrival time
    fn update_existence_time(&mut self) {

        // Update the running process
        if let Some(mut pcb) = self.running {
            pcb.total_time = self.timestamp.get() - pcb.arrival_time.get() - 1;
            self.running = Some(pcb);
        }

        // Update the processes from ready queue
        for item in self.ready.iter_mut() {
            item.total_time = self.timestamp.get() - item.arrival_time.get() - 1;

        }


        // Update the processes from sleeping vec
        for item in self.sleeping.iter_mut() {
            item.0.total_time = self.timestamp.get() - item.0.arrival_time.get() - 1;
        }

        // Update the processes from waiting vec
        for item in self.waiting.iter_mut() {
            item.0.total_time = self.timestamp.get() - item.0.arrival_time.get() - 1;
        }
    }

    /// Updates the sleeping time of all the processes from sleeping queue
    fn update_sleeping_times(&mut self) {
        
        let curr_time = self.timestamp;

        for item in self.sleeping.iter_mut() {
            let time_diff = curr_time.get() - item.1.get();
            if time_diff > item.2 {
                item.2 = 0;
            }
        }   
    }

    /// Creates a new Process Control Block related to a new process, that uses the
    /// `next_pid` as pid
    ///
    /// Function calls [`inc_pid`] to update `next_pid`, after creating the process
    /// control block
    /// 
    /// * `priority` - the priority of the new process
    /// * `timestamp` - the arrival time of the forked process
	fn spawn_process(&mut self, priority: i8, timestamp: Timestamp) -> RoundRobinPCB {
		let new_proc = RoundRobinPCB::new(self.next_pid, priority, timestamp);
		self.inc_pid();

		new_proc
	}

    /// Returns a new Round Robin process block, forked from the running process
    /// and pushes it to the `ready` queue
    /// 
    /// * `priority` - the priority of the new process
    /// * `timestamp` - the time when process is created
    fn fork(&mut self, priority: i8, timestamp: Timestamp) -> Pid {
        /* Spawn a new process and add it to the `ready` queue */
        let new_proc = self.spawn_process(priority, timestamp);
        self.ready.push_back(new_proc);

        // Fork is a syscall so it consumes one unit of time
        self.make_timeskip(1);
        new_proc.pid
    }

    /// It kills the running process and returns Success, if it exists, otherwise
    /// it does nothing and returns NoRunningProcess
    fn kill_running_process(&mut self) -> SyscallResult {
        if let Some(proc) = self.running {
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

    /// Adds the process to the sleeping queue of the scheduler
    /// 
    /// * `running` - the process to be put in the sleeping queue;
    ///             it should be the running process of the scheduler
    /// * `time` -  the time that process has to sleep
    fn send_process_to_sleep(&mut self, mut running: RoundRobinPCB, time: usize) {
        running.set_sleeping();
        self.sleeping.push((running, self.timestamp, time));
        
        // Sleep syscall consumes 1 unit of time
        self.make_timeskip(1);
    }

    /// Adds a process to the waiting queue of the scheduler
    /// 
    /// * `running` - the process to be put in the waiting queue;
    ///             if `running` isn't the scheduler's running process,
    ///             it will activate an undefined behaviour
    /// * `event` - the event that process is waiting for
    fn block_process(&mut self, mut running: RoundRobinPCB, event: Event) {
        running.wait_for_event(event);
        self.waiting.push((running, event));

        // Wait syscall consumes 1 unit of time
        self.make_timeskip(1);
    }

    /// Removes from waiting queue the processes that waited for the `event`
    /// to happen
    /// 
    /// * `event` - the event provided by a `Signal(event)` syscall
    fn unblock_processes(&mut self, event: Event) {
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

        // Signal syscall consumes 1 unit of time
        self.make_timeskip(1);
    }

    /// Sends to `ready` queue the processes that finised their sleep time
    fn awake_processes(&mut self) {
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
            let remaining = ending_timestamp.get() - self.timestamp.get() + 1;
            if remaining < min_time {
                min_time = remaining;
            }
        }

        min_time
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

        if self.running.is_some() {
            return true;
        }

        false
    }

    /// Checks if the scheduler has to sleep, is deadlocked, or all the processes are done,
    /// and returns a SchedulingDecision, otherwise returns None
    fn is_blocked(&self) -> Option<SchedulingDecision> {
        // If there is a running process, the is_blocked functon should not be called,
        // based on the flow and logic of the program, but as a measure of safety, it'll
        // return None
        if self.running.is_some() {
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

        None
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
            self.make_timeskip(self.slept_time);
            self.slept_time = 0;
        }

        // TODO: add comment that it uses those 2
        self.update_sleeping_times();
        self.awake_processes();
    }
}

impl Collector for RoundRobinScheduler {
    fn collect_running(&self) -> Vec<&dyn Process> {
        let mut proc: Vec<&dyn Process> = Vec::new();

        match &self.running {
            Some(pcb) => proc.push(pcb),
            None => (),
        }

        proc
    }

    fn collect_ready(&self) -> Vec<&dyn Process> {
        let mut procs: Vec<&dyn Process> = Vec::new();

        for item in self.ready.iter() {
            procs.push(item);
        }

        procs
    }

    fn collect_sleeping(&self) -> Vec<&dyn Process> {
        let mut procs: Vec<&dyn Process> = Vec::new();

        for item in self.sleeping.iter() {
            procs.push(&item.0);
        }

        procs
    }

    fn collect_waiting(&self) -> Vec<&dyn Process> {
        let mut procs: Vec<&dyn Process> = Vec::new();

        for item in self.waiting.iter() {
            procs.push(&item.0);
        }

        procs
    }
}

impl Scheduler for RoundRobinScheduler {
    fn stop(&mut self, reason: StopReason) -> SyscallResult {
        // Process was stopped by a syscall
        if let StopReason::Syscall { syscall, remaining } = reason {

            if let Syscall::Fork(prio) = syscall {
                let new_proc: Pid;

                if let Some(mut pcb) = self.running {
                    // Fork from a parent process

                    // Update the timings
                    let time = pcb.get_interrupted(remaining, reason);
                    self.running = Some(pcb);
                    
                    // Update the timestamp
                    self.make_timeskip(time);

                    // Spawn the new process
                    new_proc = self.fork(prio, self.timestamp);
                } else {
                    // The Fork that creates the process with PID 1 at timestamp 0

                    // Spawn the process with PID 1 at time 0
                    new_proc = self.fork(prio, self.timestamp);
                }

                return SyscallResult::Pid(new_proc);
            }

            if let Syscall::Sleep(time) = syscall {
                if let Some(mut pcb) = self.running {   
                    // Update the timings anf the timestamp
                    let passed_time = pcb.get_interrupted(remaining, reason);
                    self.make_timeskip(passed_time);

                    // Sleep the process
                    self.send_process_to_sleep(pcb, time);
                    
                    // Set running to None
                    self.running = None;
                    return SyscallResult::Success;
                }
            }

            if let Syscall::Wait(event) = syscall {
                if let Some(mut pcb) = self.running {
                    // Update the timings and the timestamp
                    let time = pcb.get_interrupted(remaining, reason);
                    self.make_timeskip(time);

                    // Make the process wait for event
                    self.block_process(pcb, Event::new(event));

                    self.running = None;
                    return SyscallResult::Success;
                }
            }

            if let Syscall::Signal(event) = syscall {
                if let Some(mut pcb) = self.running {
                    // Update the timings and the timestamp
                    let time = pcb.get_interrupted(remaining, reason);
                    self.make_timeskip(time);

                    // Unblock all processes waiting for the event to happen
                    self.unblock_processes(Event::new(event));
                    
                    self.running = Some(pcb);
                    return SyscallResult::Success;
                }
            }

            if let Syscall::Exit = syscall {
                if let Some(pcb) = self.running {
                    self.make_timeskip(pcb.time_payload - remaining);
                }
                
                return self.kill_running_process();
            }
        }

        // Process consumed it's quanta and is preempted
        if let Some(mut pcb) = self.running {
            // Update the timings
            let passed_time = pcb.get_interrupted(0, reason);

            // Update the timestamp
            self.make_timeskip(passed_time);

            self.running = Some(pcb);
            return SyscallResult::Success;
        }

        SyscallResult::NoRunningProcess
    }

    fn next(&mut self) -> SchedulingDecision {
        // Wake up the scheduler
        self.wakeup_myself();

        // If the last running process expired, then running is None
        if self.running.is_none() {
            
            // If there is no running process, at least one of the queues aren't empty,
            // and the parent process (PID 1) was killed, then there is a panic
            if self.is_panicd()  {
                return SchedulingDecision::Panic;
            }
            
            self.dequeue_ready_process();

            // If there was at least one process in the ready queue 
            if let Some(mut pcb) = self.running {
                // Run the process
                pcb.set_running();
                pcb.load_payload(self.quanta.get());
                self.running = Some(pcb);
                
                // We just set running to something, so unwrap won't give a panic
                return SchedulingDecision::Run {
                    pid: self.running.unwrap().pid,
                    timeslice: NonZeroUsize::new(self.running.unwrap().time_payload).unwrap(),
                };
            }

            if let Some(result) = self.is_blocked() {
                self.decide_sleep(result);
                return result;
            }
        }

        if let Some(proc) = self.running {
            // Case 1: It cannot continue running
            if proc.time_payload < self.min_time {
                self.enqueue_process(proc);

                self.dequeue_ready_process();

                if let Some(mut ready_proc) = self.running {
                    ready_proc.set_running();
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

        SchedulingDecision::Done

    }

    fn list(&mut self) -> Vec<&dyn Process> {
        self.update_existence_time();
        return collect_all(self);
    }
}
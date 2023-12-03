use std::num::NonZeroUsize;
use std::ops::Add;
use std::collections::{VecDeque, HashMap};

use crate::common_types::{Timestamp, Event, MIN_PRIO, MAX_PRIO};
use crate::collector;
use crate::scheduler::{Pid, ProcessState, Process, SchedulingDecision, StopReason,
SyscallResult, Scheduler};
use crate::Syscall;


/// The Priority Round Robin process control block
#[derive(Clone, Copy)]
pub struct PrioRoundRobinPCB {
    /// The PID of the process
    pid: Pid,
    /// The priority that process had when it was created
    /// 
    /// The priority of the process will never exceed this priority
    /// It cannot be greater than 5
    priority_at_born: i8,
    /// The priority of the process
    /// 
    /// The priority can't be smaller than 0, or greater than `priority_at_born`
    priority: i8,
    /// The state of the process
    state: ProcessState,
    /// The timestamp when process was spawned
    arrival_time: Timestamp,
    /// The total time of existence
    total_time: usize,
    /// The time that process spent executing instructions
    exec_time: usize,
    /// The time that process consumed on syscalls, or the
    /// number of syscalls that interrupted the process in all of
    /// his existence
    syscall_time: usize,
    /// The maximum time that process can execute instructions, before being
    /// preempted
    time_payload: usize,
}

impl PrioRoundRobinPCB {
    /// Creates a new Process Control Block
    /// 
    /// * `pid` - PID of the new process
    /// * `priority` - priority of the new process
    /// * `arrival` - timestamp when process is created
    fn new(pid: Pid, priority: i8, arrival: Timestamp) -> PrioRoundRobinPCB {
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


pub struct PriorityRRScheduler {
    /// Map with the ready processes for each priority
    ready: HashMap<i8, VecDeque<PrioRoundRobinPCB>>,
    /// Sleeping queue, common for all priorities
    sleep: Vec<(PrioRoundRobinPCB, Timestamp)>,
    /// Waiting queue, common for all priorities
    waiting: Vec<(PrioRoundRobinPCB, Event)>,
    /// Running process
    running: Option<PrioRoundRobinPCB>,
    /// Time quanta of the scheduler
    /// 
    /// The maximum time a process can run before being preempted
    quanta: NonZeroUsize,
    /// The minimum remaining time a process needs for being replainfied just after
    /// a syscall
    /// 
    /// TODO: add explanation from round robin
    min_time: usize,
    /// The pid of the next process that will be spawned
    next_pid: Pid,
    /// Current time (since process with PID 1 started)
    timestamp: Timestamp,
    /// Panic indicator
    /// 
    /// If process with PID 1 is killed before it's children, panic is activated
    panicd: bool,
    /// Time that scheduler has slept
    slept_time: usize,
}

impl PriorityRRScheduler {
    pub fn new(timeslice: NonZeroUsize,
        minimum_remaining_timeslice: usize)
        -> PriorityRRScheduler {
        
        PriorityRRScheduler {
            ready: HashMap::new(),
            sleep: Vec::new(),
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
}

impl Scheduler for PriorityRRScheduler {
    
    fn stop(&mut self, reason: StopReason) -> SyscallResult {
        SyscallResult::NoRunningProcess
    }

    fn next(&mut self) -> SchedulingDecision {
        SchedulingDecision::Done
    }

    fn list(&mut self) -> Vec<&dyn Process> {
        let procs: Vec<&dyn Process> = Vec::new();    
        procs
    }
}
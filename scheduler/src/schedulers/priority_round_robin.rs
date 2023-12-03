use std::num::NonZeroUsize;
use std::ops::Add;
use std::collections::{VecDeque, HashMap};

use crate::{PrioRoundRobinPCB, ProcessControlBlock};
use crate::common_types::{Timestamp, Event, MIN_PRIO, MAX_PRIO};
use crate::{Collector, collect_all};
use crate::scheduler::{Pid, ProcessState, Process, SchedulingDecision, StopReason,
SyscallResult, Scheduler};
use crate::Syscall;


pub struct PriorityRRScheduler {
    /// Map with the ready processes for each priority
    ready: HashMap<i8, VecDeque<PrioRoundRobinPCB>>,
    /// Sleeping queue, common for all priorities
    sleeping: Vec<(PrioRoundRobinPCB, Timestamp, usize)>,
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

    fn update_existence_time(&mut self) {
        if let Some(mut pcb) = self.running {
            pcb.total_time = self.timestamp.get() - pcb.arrival_time.get() - 1;
            self.running = Some(pcb);
        }

        for i in (MIN_PRIO..=MAX_PRIO).rev() {
            if let Some(queue) = self.ready.get_mut(&i) {
                for item in queue.iter_mut() {
                    item.total_time = self.timestamp.get() - item.arrival_time.get() - 1;
                }

            }
        }

        for item in self.sleeping.iter_mut() {
            item.0.total_time = self.timestamp.get() - item.0.arrival_time.get() - 1;
        }

        for item in self.waiting.iter_mut() {
            item.0.total_time = self.timestamp.get() - item.0.arrival_time.get() - 1;
        }
    }

    fn make_timeskip(&mut self, time: usize) {
        self.timestamp = self.timestamp.add(time);
    }

    fn inc_pid(&mut self) {
        self.next_pid = self.next_pid.add(1);
    }

    fn spawn_process(&mut self, priority: i8, timestamp: Timestamp) -> PrioRoundRobinPCB {
        let new_proc = PrioRoundRobinPCB::new(self.next_pid, priority, timestamp);
        self.inc_pid();

        new_proc
    }

    fn fork(&mut self, priority: i8, timestamp: Timestamp) -> Pid {
        let new_proc = self.spawn_process(priority, timestamp);
        self.enqueue_process(new_proc);

        new_proc.pid()
    }

    fn enqueue_process(&mut self, mut proc: PrioRoundRobinPCB) {
        proc.set_state(ProcessState::Ready);

        let prio = proc.priority();
        
        if let Some(queue) = self.ready.get_mut(&prio) {
            queue.push_back(proc);
            return;
        }

        let mut queue: VecDeque<PrioRoundRobinPCB> = VecDeque::new();
        queue.push_back(proc);
        self.ready.insert(prio, queue);
    }

    fn dequeue_process(&mut self) {
        for i in (MIN_PRIO..=MAX_PRIO).rev() {
            if let Some(queue) = self.ready.get_mut(&i) {
                if queue.is_empty() {
                    continue;
                }

                self.running = queue.pop_front();
                return;
            }
        }
    }

    fn kill_running(&mut self) -> SyscallResult {
        return if let Some(proc) = self.running {
            if proc.pid == Pid::new(1) {
                self.panicd = true;
            }

            self.running = None;
            SyscallResult::Success
        } else {
            SyscallResult::NoRunningProcess
        }
    }

    fn interrupt_process(&self,
        running: &mut PrioRoundRobinPCB,
        remaining: usize,
        reason: StopReason)
        -> usize {
        
        let exec_time: usize;

        if let StopReason::Syscall { .. } = reason {
            running.syscall();
            exec_time = running.time_payload - remaining - 1;
        } else {
            exec_time = running.time_payload - remaining;
        }

        running.execute(exec_time);
        running.load_payload(remaining);

        return  exec_time;
    }
    
}

impl Collector for PriorityRRScheduler {
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

        for (_, queue) in self.ready.iter() {
            for item in queue.iter() {
                procs.push(item);
            }
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

impl Scheduler for PriorityRRScheduler{
    
    fn stop(&mut self, reason: StopReason) -> SyscallResult {
        
        if let StopReason::Syscall { syscall, remaining } = reason {
            let new_proc_pid: Pid;

            if let Syscall::Fork(prio) = syscall {
                if let Some(mut pcb) = self.running {
                    let passed_time = self.interrupt_process(&mut pcb, remaining, reason);
                    self.running = Some(pcb);

                    self.make_timeskip(passed_time);

                    new_proc_pid = self.fork(prio, self.timestamp);
                    self.make_timeskip(1);
                } else {
                    new_proc_pid = self.fork(prio, self.timestamp);
                    self.make_timeskip(1);
                }

                return SyscallResult::Pid(new_proc_pid);
            }

            if let Syscall::Exit = syscall {
                if let Some(pcb) = self.running {
                    self.make_timeskip(pcb.time_payload - remaining);
                }

                return self.kill_running();
            }
        }

        if let Some(mut pcb) = self.running {
            let passed_time = self.interrupt_process(&mut pcb, 0, reason);

            self.make_timeskip(passed_time);
            self.running = Some(pcb);

            return SyscallResult::Success;
        }
        
        
        SyscallResult::NoRunningProcess
    }

    fn next(&mut self) -> SchedulingDecision {
        if let None = self.running {
            self.dequeue_process();

            if let Some(mut pcb) = self.running {
                pcb.set_running();
                pcb.load_payload(self.quanta.get());
                self.running = Some(pcb);

                return SchedulingDecision::Run {
                    pid: self.running.unwrap().pid,
                    timeslice: self.quanta
                };
            } else {
                return SchedulingDecision::Done;
            }
        }

        if let Some(proc) = self.running {
            if proc.time_payload < self.min_time {
                self.enqueue_process(proc);

                self.dequeue_process();

                if let Some(mut ready_proc) = self.running {
                    ready_proc.set_running();
                    ready_proc.load_payload(self.quanta.get());
                    self.running = Some(ready_proc);
                }

                return SchedulingDecision::Run {
                    pid: self.running.unwrap().pid,
                    timeslice: self.quanta,
                };
            }

            return SchedulingDecision::Run {
                pid: proc.pid,
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
use std::num::NonZeroUsize;
use std::ops::Add;
use std::collections::{VecDeque, HashMap};

use crate::{PriorityQueuePCB, ProcessControlBlock};
use crate::common_types::{Timestamp, Event, MIN_PRIO, MAX_PRIO};
use crate::{Collector, collect_all};
use crate::scheduler::{Pid, ProcessState, Process, SchedulingDecision, StopReason,
SyscallResult, Scheduler};
use crate::Syscall;


pub struct PriorityQueueScheduler {
    /// Map with the ready processes for each priority
    ready: HashMap<i8, VecDeque<PriorityQueuePCB>>,
    /// Sleeping queue, common for all priorities
    sleeping: Vec<(PriorityQueuePCB, Timestamp, usize)>,
    /// Waiting queue, common for all priorities
    waiting: Vec<(PriorityQueuePCB, Event)>,
    /// Running process
    running: Option<PriorityQueuePCB>,
    /// Time quanta of the scheduler
    /// 
    /// The maximum time a process can run before being preempted
    quanta: NonZeroUsize,
    /// The minimum remaining time a process needs for being replainfied just after
    /// a syscall
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

impl PriorityQueueScheduler {
    pub fn new(timeslice: NonZeroUsize,
        minimum_remaining_timeslice: usize)
        -> PriorityQueueScheduler {
        
        PriorityQueueScheduler {
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

    fn update_sleeping_times(&mut self) {
        let curr_time = self.timestamp.get();

        for item in self.sleeping.iter_mut() {
            let time_diff = curr_time - item.1.get();
            if time_diff > item.2 {
                item.2 = 0;
            }
        }
    }

    fn make_timeskip(&mut self, time: usize) {
        self.timestamp = self.timestamp.add(time);
    }

    fn inc_pid(&mut self) {
        self.next_pid = self.next_pid.add(1);
    }

    fn spawn_process(&mut self, priority: i8, timestamp: Timestamp) -> PriorityQueuePCB {
        let new_proc = PriorityQueuePCB::new(self.next_pid, priority, timestamp);
        self.inc_pid();

        new_proc
    }

    fn fork(&mut self, priority: i8, timestamp: Timestamp) -> Pid {
        let new_proc = self.spawn_process(priority, timestamp);
        self.enqueue_process(new_proc);

        self.make_timeskip(1);

        new_proc.pid
    }

    fn enqueue_process(&mut self, mut proc: PriorityQueuePCB) {
        proc.set_state(ProcessState::Ready);

        let prio = proc.priority();
        
        if let Some(queue) = self.ready.get_mut(&prio) {
            queue.push_back(proc);
            return;
        }

        let mut queue: VecDeque<PriorityQueuePCB> = VecDeque::new();
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
        if let Some(proc) = self.running {
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
        running: &mut PriorityQueuePCB,
        remaining: usize,
        reason: StopReason)
        -> usize {
        
        let exec_time: usize;

        if let StopReason::Syscall { .. } = reason {
            running.syscall();
            running.inc_priority();
            exec_time = running.time_payload - remaining - 1;
        } else {
            exec_time = running.time_payload - remaining;
            running.dec_priority();
        }

        running.execute(exec_time);
        running.load_payload(remaining);

        exec_time
    }

    fn send_process_to_sleep(&mut self, mut proc: PriorityQueuePCB, time: usize) {
        proc.set_sleeping();

        self.sleeping.push((proc, self.timestamp, time));
        self.make_timeskip(1);
    }

    fn awake_processes(&mut self) {
        let mut procs: VecDeque<PriorityQueuePCB> = VecDeque::new();

        self.sleeping.retain(|item| {
            if item.2 == 0 {
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

    fn block_process(&mut self, mut proc: PriorityQueuePCB, event: Event) {
        proc.wait_for_event(event);
        self.waiting.push((proc, event));

        self.make_timeskip(1);
    }

    fn unblock_processes(&mut self, event: Event) {
        let mut procs: VecDeque<PriorityQueuePCB> = VecDeque::new();

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

        self.make_timeskip(1);
    }

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

    fn has_ready_processes(&self) -> bool {
        for i in (MIN_PRIO..=MAX_PRIO).rev() {
            if let Some(queue) = self.ready.get(&i) {
                if !queue.is_empty() {
                    return true;
                }
            }
        }

        false
    }

    fn is_panicd(&self) -> bool {
        if !self.panicd {
            return false;
        }

        if self.has_ready_processes() || !self.sleeping.is_empty() || !self.waiting.is_empty() {
            return true;
        }

        if self.running.is_some() {
            return true;
        }

        false
    }

    fn is_blocked(&self) -> Option<SchedulingDecision> {
        if self.running.is_some() {
            return None;
        }

        if self.has_ready_processes() {
            return None;
        }

        if self.sleeping.is_empty() && self.waiting.is_empty() {
            return Some(SchedulingDecision::Done);
        }

        if self.sleeping.is_empty() && !self.waiting.is_empty() {
            return Some(SchedulingDecision::Deadlock);
        }

        if !self.sleeping.is_empty() {
            let sleep_time = self.get_sleep_time();

            return Some(SchedulingDecision::Sleep(NonZeroUsize::new(sleep_time).unwrap()));
        }

        None
    }

    fn give_time_to_sleep(&mut self, decision: SchedulingDecision) {
        if let SchedulingDecision::Sleep(time) = decision {
            self.slept_time = time.get();
        } else {
            self.slept_time = 0;
        }
    }

    fn wakeup_myself(&mut self) {
        if self.slept_time != 0 {
            self.make_timeskip(self.slept_time);
            self.slept_time = 0;
        }

        self.update_sleeping_times();
        self.awake_processes();
    }
    
}

impl Collector for PriorityQueueScheduler {
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

impl Scheduler for PriorityQueueScheduler{
    
    fn stop(&mut self, reason: StopReason) -> SyscallResult {
        
        if let StopReason::Syscall { syscall, remaining } = reason {
            let new_proc_pid: Pid;

            if let Syscall::Fork(prio) = syscall {
                if let Some(mut pcb) = self.running {
                    let time = pcb.get_interrupted(remaining, reason);
                    self.running = Some(pcb);

                    self.make_timeskip(time);

                    new_proc_pid = self.fork(prio, self.timestamp);
                } else {
                    new_proc_pid = self.fork(prio, self.timestamp);
                }

                return SyscallResult::Pid(new_proc_pid);
            }

            if let Syscall::Sleep(time) = syscall {
                if let Some(mut pcb) = self.running {
                    let passed_time = pcb.get_interrupted(remaining, reason);
                    self.make_timeskip(passed_time);

                    self.send_process_to_sleep(pcb, time);
                    
                    self.running = None;
                    return SyscallResult::Success;
                }
            }

            if let Syscall::Wait(event) = syscall {
                if let Some(mut pcb) = self.running {
                    let passed_time = self.interrupt_process(&mut pcb, remaining, reason);
                    self.make_timeskip(passed_time);

                    self.block_process(pcb, Event::new(event));

                    self.running = None;
                    return SyscallResult::Success;
                }
            }

            if let Syscall::Signal(event) = syscall {
                if let Some(mut pcb) = self.running {
                    let time = self.interrupt_process( &mut pcb, remaining, reason);
                    self.make_timeskip(time);

                    self.unblock_processes(Event::new(event));

                    self.running = Some(pcb);
                    return SyscallResult::Success;
                }
            }

            if let Syscall::Exit = syscall {
                if let Some(pcb) = self.running {
                    self.make_timeskip(pcb.time_payload - remaining);
                }

                return self.kill_running();
            }
        }

        if let Some(mut pcb) = self.running {
            let time = self.interrupt_process(&mut pcb, 0, reason);

            self.make_timeskip(time);
            self.running = Some(pcb);

            return SyscallResult::Success;
        }
        
        
        SyscallResult::NoRunningProcess
    }

    fn next(&mut self) -> SchedulingDecision {
        self.wakeup_myself();
        
        if self.running.is_none() {
            if self.is_panicd() {
                return SchedulingDecision::Panic;
            }
            
            self.dequeue_process();

            if let Some(mut pcb) = self.running {
                pcb.set_running();
                pcb.load_payload(self.quanta.get());
                self.running = Some(pcb);

                return SchedulingDecision::Run {
                    pid: self.running.unwrap().pid,
                    timeslice: self.quanta
                };
            }

            if let Some(result) = self.is_blocked() {
                self.give_time_to_sleep(result);
                return result;
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
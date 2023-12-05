use std::collections::VecDeque;
use std::num::NonZeroUsize;
use std::ops::Add;

use crate::FairPCB;
use crate::Syscall;
use crate::Timestamp;
use crate::Event;
use crate::Vruntime;
use crate::{Scheduler, Process, ProcessControlBlock, Collector, collect_all};
use crate::{Pid, StopReason, SyscallResult, SchedulingDecision, ProcessState};

pub struct FairScheduler {
    ready: Vec<FairPCB>,
    sleeping: Vec<(FairPCB, Timestamp, usize)>,
    waiting: Vec<(FairPCB, Event)>,
    running: Option<FairPCB>,
    cpu_time: NonZeroUsize,
    quanta: NonZeroUsize,
    min_timeslice: usize,
    next_pid: Pid,
    timestamp: Timestamp,
    panicd: bool,
    slept_time: usize,
    proc_number: usize,
}

impl FairScheduler {
    pub fn new(cpu_time: NonZeroUsize, minimum_remaining_timeslice: usize) -> FairScheduler {
        FairScheduler {
            ready: Vec::new(),
            sleeping: Vec::new(),
            waiting: Vec::new(),
            running: None,
            cpu_time,
            quanta: cpu_time,
            min_timeslice: minimum_remaining_timeslice,
            next_pid: Pid::new(1),
            timestamp: Timestamp::new(0),
            panicd: false,
            slept_time: 0,
            proc_number: 0,
        }
    }

    fn update_existence_time(&mut self) {
        if let Some(mut pcb) = self.running {
            pcb.total_time = self.timestamp.get() - pcb.arrival_time.get() - 1;
            self.running = Some(pcb);
        }

        for item in self.ready.iter_mut() {
            item.total_time = self.timestamp.get() - item.arrival_time.get() - 1;
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

    fn inc_number(&mut self) {
        self.proc_number += 1;
    }

    fn dec_number(&mut self) {
        self.proc_number -= 1;
    }

    fn recalculate_quanta(&mut self) {
        if self.proc_number == 0 {
            return
        }

        let new_quanta = NonZeroUsize::new(self.cpu_time.get() / self.proc_number).unwrap();
        self.quanta = new_quanta;
    }

    fn get_min_viruntime(&self) -> Option<Vruntime> {
        
        if self.running.is_none() && self.ready.is_empty() &&
           self.sleeping.is_empty() && self.waiting.is_empty() {
            
            return None;
        };
        
        let mut min = Vruntime::new(usize::MAX);

        if let Some(pcb) = self.running {
            if pcb < min {
                min = pcb.vruntime;
            }
        }

        for item in self.ready.iter() {
            if *item < min {
                min = item.vruntime;
            }
        }

        for item in self.sleeping.iter() {
            if item.0 < min {
                min = item.0.vruntime
            }
        }

        for item in self.waiting.iter() {
            if item.0 < min {
                min = item.0.vruntime
            }
        }

        Some(min) 
    }

    fn get_next_process(&mut self) -> Option<FairPCB> {
        let mut process: Option<FairPCB> = None;
        let mut min_vruntime = Vruntime::new(usize::MAX);
        let mut min_pid = Pid::new(usize::MAX);

        for item in self.ready.iter() {
            if item < &min_vruntime {
                process = Some(*item);
                min_vruntime = item.vruntime;
                min_pid = item.pid;
            }

            if item == &min_vruntime && item.pid < min_pid {
                process = Some(*item);
                min_pid = item.pid;
            }
        }

        if let Some(pcb) = process {
            self.ready.remove(self.ready.iter().position(|item| *item == pcb).unwrap());
        }

        process
    }

    fn spawn_process(&mut self,
        priority: i8,
        timestamp: Timestamp,
        vruntime: Vruntime) -> FairPCB {
        
        let new_proc = FairPCB::new(self.next_pid, priority, timestamp, vruntime);
        self.inc_pid();

        new_proc
    }

    fn fork(&mut self, priority: i8, timestamp: Timestamp) -> Pid {
        let vruntime = self.get_min_viruntime();
        let new_proc: FairPCB = match vruntime {
            Some(time) => self.spawn_process(priority, timestamp, time),
            None => self.spawn_process(priority, timestamp, Vruntime::new(0)),
        };
        
        self.inc_number();
        self.recalculate_quanta();
    
        self.enqueue_process(new_proc);

        new_proc.pid
    }

    fn interrupt_process(&self,
        proc: &mut FairPCB,
        remaining: usize,
        reason: StopReason) -> usize {
        
        let exec_time: usize;

        if let StopReason::Syscall { .. } = reason {
            proc.syscall();
            exec_time = proc.time_payload - remaining - 1;
            proc.vruntime = proc.vruntime.add(exec_time + 1);
        } else {
            exec_time = proc.time_payload - remaining;
            proc.vruntime = proc.vruntime.add(exec_time);
        }

        proc.execute(exec_time);
        proc.load_payload(remaining);

        exec_time
    }

    fn enqueue_process(&mut self, mut proc: FairPCB) {
        proc.set_state(ProcessState::Ready);

        self.ready.push(proc);
    }

    fn dequeue_process(&mut self) {
        self.running = self.get_next_process();
    }

    fn send_process_to_sleep(&mut self, mut proc: FairPCB, time: usize) {
        proc.set_sleeping();
        self.sleeping.push((proc, self.timestamp, time));

        self.dec_number();
        self.recalculate_quanta();
    }

    fn awake_processes(&mut self) {
        let mut procs: VecDeque<FairPCB> = VecDeque::new();

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
            self.inc_number();
        }

        self.recalculate_quanta();
    }

    fn block_process(&mut self, mut proc: FairPCB, event: Event) {
        proc.wait_for_event(event);
        self.waiting.push((proc, event));

        self.dec_number();
        self.recalculate_quanta();
    }

    fn unblock_processes(&mut self, event: Event) {
        let mut procs: VecDeque<FairPCB> = VecDeque::new();

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
            self.inc_number();
        }

        self.recalculate_quanta();
    }

    fn update_sleeping_times(&mut self) {
        let curr_time = self.timestamp;

        for item in self.sleeping.iter_mut() {
            let time_diff = curr_time.get() - item.1.get();
            if time_diff > item.2 {
                item.2 = 0;
            }
        }
    }

    fn kill_running(&mut self) -> SyscallResult{
        if let Some(proc) = self.running {
            if proc.pid == 1 {
                self.panicd = true;
            }

            self.dec_number();
            self.running = None;
            self.recalculate_quanta();
            SyscallResult::Success
        } else {
            SyscallResult::NoRunningProcess
        }
    }

    fn decide_sleep(&mut self, decision: SchedulingDecision) {
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

    fn is_blocked(&self) -> Option<SchedulingDecision> {
        if self.running.is_some() {
            return None;
        }

        if !self.ready.is_empty() {
            return None;
        }

        if self.sleeping.is_empty() && self.waiting.is_empty() {
            return Some(SchedulingDecision::Done);
        }

        if self.sleeping.is_empty() && !self.waiting.is_empty() {
            return Some(SchedulingDecision::Deadlock);
        }

        if !self.sleeping.is_empty() {
            // TODO: explain why this is not 0
            let sleeping_time = self.get_sleep_time();
            
            return Some(SchedulingDecision::Sleep(NonZeroUsize::new(sleeping_time).unwrap()));
        }

        None
    }

}

impl Collector for FairScheduler {
    fn collect_running(&self) -> Vec<&dyn Process> {
        let mut proc: Vec<&dyn Process> = Vec::new();

        match &self.running {
            Some(pcb) => proc.push(pcb),
            None => (),
        };

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


impl Scheduler for FairScheduler {
    fn stop(&mut self, reason: StopReason) -> SyscallResult {
        if let StopReason::Syscall { syscall, remaining } = reason {

            if let Syscall::Fork(prio) = syscall {
                let new_proc_pid: Pid;

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
            
            if let Syscall::Sleep(time) = syscall {
                if let Some(mut pcb) = self.running {
                    let passed_time = self.interrupt_process(&mut pcb, remaining, reason);
                    self.make_timeskip(passed_time);
                    
                    self.send_process_to_sleep(pcb, time);
                    self.make_timeskip(1);
                    
                    self.running = None;
                    return SyscallResult::Success;
                }
            }
            
            if let Syscall::Wait(event) = syscall {
                if let Some(mut pcb) = self.running {
                    let passed_time = self.interrupt_process(&mut pcb, remaining, reason);
                    self.make_timeskip(passed_time);

                    self.block_process(pcb, Event::new(event));
                    self.make_timeskip(1);

                    self.running = None;
                    return SyscallResult::Success;
                }
            }

            if let Syscall::Signal(event) = syscall {
                if let Some(mut pcb) = self.running {
                    let passed_time = self.interrupt_process(&mut pcb, remaining, reason);
                    self.make_timeskip(passed_time);

                    self.unblock_processes(Event::new(event));
                    self.make_timeskip(1);

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
            
            let passed_time = self.interrupt_process(&mut pcb, 0, StopReason::Expired);
            self.make_timeskip(passed_time);

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
                    timeslice: self.quanta,
                };
            }

            if let Some(result) = self.is_blocked() {
                self.decide_sleep(result);
                return result;
            }

            panic!("Fatal error!");
        }

        if let Some(mut proc) = self.running {
            if proc.time_payload > self.quanta.get() {
                proc.time_payload = self.quanta.get();
                self.running = Some(proc);
            }
            
            if proc.time_payload < self.min_timeslice {
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

        panic!("Fatal error");
    }

    fn list(&mut self) -> Vec<&dyn Process> {
        self.update_existence_time();
        return collect_all(self);
    }
}
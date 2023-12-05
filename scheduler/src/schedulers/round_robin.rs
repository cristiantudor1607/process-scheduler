
use std::{collections::VecDeque, num::NonZeroUsize, ops::Add};

use crate::{GeneralProcess, Timestamp, Event, Pid, SchedulerInfo, ProcessManager, ProcessState, Vruntime, ProcessControlBlock, scheduler_info::SchedulerActions, Collector, Process, Scheduler, collect_all, SchedulingDecision, SyscallResult, StopReason, Syscall, execute_fork, execute_exit, execute_expired};

pub struct RoundRobinScheduler {
    ready: VecDeque<GeneralProcess>,
    sleeping: Vec<(GeneralProcess, Timestamp, usize)>,
    waiting: Vec<(GeneralProcess, Event)>,
    running: Option<GeneralProcess>,
    quanta: NonZeroUsize,
    min_time: usize,
    next_pid: Pid,
    timestamp: Timestamp,
    panicd: bool,
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
            slept_time: 0
        }
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

impl SchedulerInfo for RoundRobinScheduler {
    fn get_timestamp(&self) -> Timestamp {
        self.timestamp
    }

    fn make_timeskip(&mut self, time: usize) {
        self.timestamp = self.timestamp.add(time);
    }

    fn get_next_pid(&self) -> Pid {
        self.next_pid
    }

    fn inc_next_pid(&mut self) {
        self.next_pid = self.next_pid.add(1);
    }

    fn get_running(&self) -> Option<GeneralProcess> {
        self.running
    }

    fn set_running(&mut self, proc: Option<GeneralProcess>) {
        self.running = proc;
    }

    fn get_panicd(&self) -> bool {
        self.panicd
    }

    fn set_panicd(&mut self) {
        self.panicd = true;
    }
    
    fn reset_panicd(&mut self) {
        self.panicd = false;
    }

    fn get_slept_time(&self) -> usize {
        self.slept_time
    }

    fn set_slept_time(&mut self, time: usize) {
        self.slept_time = time;
    }

    fn get_quanta(&self) -> NonZeroUsize {
        self.quanta
    }

    fn get_next_process(&mut self) -> Option<GeneralProcess> {
        self.ready.pop_front()
    }

    fn push_to_sleeping_queue(&mut self, proc: GeneralProcess, time: usize) {
        self.sleeping.push((proc, self.timestamp, time));
    }

    fn push_to_waiting_queue(&mut self, proc: GeneralProcess, event: Event) {
        self.waiting.push((proc, event));
    }

    fn compute_total_time(&mut self) {
        if let Some(mut pcb) = self.running {
            let time = self.timestamp.get() - pcb.get_arrival_time().get() - 1;
            pcb.set_total_time(time);
            self.running = Some(pcb);
        }

        for item in self.ready.iter_mut() {
            let time = self.timestamp.get() - item.get_arrival_time().get() - 1;
            item.set_total_time(time);
        }

        for item in self.sleeping.iter_mut() {
            let time = self.timestamp.get() - item.0.get_arrival_time().get() - 1;
            item.0.set_total_time(time);
        }

        for item in self.waiting.iter_mut() {
            let time = self.timestamp.get() - item.0.get_arrival_time().get() - 1;
            item.0.set_total_time(time);
        }
    }

    fn compute_sleeping_times(&mut self) {
        let curr_time = self.timestamp;

        for item in self.sleeping.iter_mut() {
            let time_diff = curr_time.get() - item.1.get();
            if time_diff > item.2 {
                item.2 = 0;
            }
        }
    }

    fn has_running_process(&self) -> bool {
        return if let None = self.running {
            false
        } else {
            true
        };
    }

    fn has_ready_processes(&self) -> bool {
        return if self.ready.is_empty() {
            false
        } else {
            true
        };
    }

    fn has_sleeping_processes(&self) -> bool {
        return if self.sleeping.is_empty() {
            false
        } else {
            true
        }
    }

    fn has_waiting_processes(&self) -> bool {
        return if self.waiting.is_empty() {
            false
        } else {
            true
        }
    }

    fn get_time_for_sleep(&self) -> usize {
        if !self.has_sleeping_processes() {
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

        return min_time;
    }

    fn get_min_vruntime(&self) -> Option<Vruntime> {
        None
    }

    fn inc_number(&mut self) {}
    fn dec_number(&mut self) {}
    fn recompute_quanta(&mut self) {}
}

impl ProcessManager for RoundRobinScheduler {
    fn enqueue_process(&mut self, proc: &mut GeneralProcess) {
        proc.set_state(ProcessState::Ready);

        self.ready.push_back(*proc);
    }

    fn awake_processes(&mut self) {
        let mut procs: VecDeque<GeneralProcess> = VecDeque::new();

        self.sleeping.retain(|item| {
            if item.2 == 0 {
                procs.push_back(item.0);
                false
            } else {
                true
            }
        });

        for item in procs.iter_mut() {
            self.enqueue_process(item);
            self.inc_number();
        }

        self.recompute_quanta();
    }

    fn unblock_processes(&mut self, event: Event) {
        let mut procs: VecDeque<GeneralProcess> = VecDeque::new();

        self.waiting.retain(|item| {
            if item.1 == event {
                procs.push_back(item.0);
                false
            } else {
                true
            }
        });

        for item in procs.iter_mut() {
            self.enqueue_process(item);
            self.inc_number();
        }

        self.recompute_quanta();
    }

}

impl SchedulerActions for RoundRobinScheduler {}

impl Scheduler for RoundRobinScheduler {
    fn stop(&mut self, reason: StopReason) -> SyscallResult {
        
        if let StopReason::Syscall { syscall, remaining } = reason {
            if let Syscall::Fork(prio) = syscall {
                return execute_fork(self, remaining, prio)
            }

            if let Syscall::Exit = syscall {
                return execute_exit(self, remaining);
            }

            unimplemented!("next: sleep");
        } else {
            return execute_expired(self);
        }
    }

    fn next(&mut self) -> SchedulingDecision {
        self.wakeup_myself();

        if let None = self.get_running() {

            if self.is_panicd() {
                return SchedulingDecision::Panic;
            }

            self.dequeue_process();

            if let Some(mut pcb) = self.get_running() {
                pcb.set_running();
                pcb.load_payload(self.get_quanta().get());
                self.set_running(Some(pcb));

                return SchedulingDecision::Run {
                    pid: pcb.get_pid(),
                    timeslice: self.get_quanta()
                };
            }

            if let Some(result) = self.is_blocked() {
                self.decide_sleep(result);
                return result;
            }

            panic!("This point cannot be reached by logic rules");
        }

        if let Some(mut pcb) = self.get_running() {
            if pcb.get_payload() < self.min_time {
                self.enqueue_process(&mut pcb);

                self.dequeue_process();

                if let Some(mut ready_process) = self.get_running() {
                    ready_process.set_running();
                    ready_process.load_payload(self.get_quanta().get());
                    self.set_running(Some(ready_process));

                    return SchedulingDecision::Run {
                        pid: ready_process.get_pid(),
                        timeslice: self.get_quanta()
                    };
                };

                panic!("This point cannot be reaches by logic rules");
            } else {
                let pid = pcb.get_pid();
                let timeslice: NonZeroUsize = NonZeroUsize::new(pcb.get_payload()).unwrap();
                return SchedulingDecision::Run {
                    pid,
                    timeslice
                };
            }
        }

        panic!("This point cannot be reaches by logic rules");
    
    }

    fn list(&mut self) -> Vec<&dyn Process> {
        self.compute_total_time();
        collect_all(self)
    }
}
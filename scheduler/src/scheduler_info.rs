use std::num::NonZeroUsize;

use crate::{Timestamp, Pid, Event, SyscallResult, SchedulingDecision, Vruntime, schedulers::GeneralProcess, ProcessControlBlock};

pub trait SchedulerInfo {
    fn get_timestamp(&self) -> Timestamp;
    fn make_timeskip(&mut self, time: usize);

    fn get_next_pid(&self) -> Pid;
    fn inc_next_pid(&mut self);

    fn get_running(&self) -> Option<GeneralProcess>;
    fn set_running(&mut self, proc: Option<GeneralProcess>);

    fn get_panicd(&self) -> bool;
    fn set_panicd(&mut self);
    fn reset_panicd(&mut self);

    fn get_slept_time(&self) -> usize;
    fn set_slept_time(&mut self, time: usize);

    fn get_quanta(&self) -> NonZeroUsize;

    fn get_next_process(&mut self) -> Option<GeneralProcess>;

    fn push_to_sleeping_queue(&mut self, proc: GeneralProcess, time: usize);
    fn push_to_waiting_queue(&mut self, proc: GeneralProcess, event: Event);

    fn compute_sleeping_times(&mut self);
    fn compute_total_time(&mut self);

    fn has_running_process(&self) -> bool;
    fn has_ready_processes(&self) -> bool;
    fn has_sleeping_processes(&self) -> bool;
    fn has_waiting_processes(&self) -> bool;

    fn get_time_for_sleep(&self) -> usize;
    fn get_min_vruntime(&self) -> Option<Vruntime>;

    fn inc_number(&mut self);
    fn dec_number(&mut self);
    fn recompute_quanta(&mut self);
}

pub trait ProcessManager : SchedulerInfo {
    fn spawn_process(&mut self,
        prio: i8,
        opt_vruntime: Option<Vruntime>) -> GeneralProcess {
        
        let pid = self.get_next_pid();
        let arrival_time = self.get_timestamp();
        let new_proc: GeneralProcess;
        match opt_vruntime {
            Some(vruntime) => new_proc = GeneralProcess::with_vruntime(pid, prio, arrival_time, vruntime),
            None => new_proc = GeneralProcess::new(pid, prio, arrival_time),
        }

        self.inc_next_pid();

        new_proc
    }

    fn fork(&mut self, prio: i8) -> Pid {
        let vruntime = self.get_min_vruntime();
        let pid = self.get_next_pid();
        let timestamp = self.get_timestamp();

        let mut new_proc: GeneralProcess;
        match vruntime {
            Some(virtual_runtime) => {
                new_proc = GeneralProcess::with_vruntime(pid, prio, timestamp, virtual_runtime);
            },
            None => {
                new_proc = GeneralProcess::new(pid, prio, timestamp);
            }
        }

        self.inc_number();
        self.recompute_quanta();
        self.enqueue_process(&mut new_proc);

        new_proc.get_pid()
    }
    
    fn enqueue_process(&mut self, proc: &mut GeneralProcess);

    fn dequeue_process(&mut self) {
        let process = self.get_next_process();
        self.set_running(process);
    }

    fn send_process_to_sleep(&mut self, proc: &mut GeneralProcess, time: usize) {
        proc.set_sleeping();
        self.push_to_sleeping_queue(*proc, time);

        self.dec_number();
        self.recompute_quanta();
    }
    
    fn awake_processes(&mut self);

    fn block_process(&mut self, proc: &mut GeneralProcess, event: Event) {
        proc.wait_for_event(event);
        self.push_to_waiting_queue(*proc, event);

        self.dec_number();
        self.recompute_quanta();
    }

    fn unblock_processes(&mut self, event: Event);

    fn kill_running(&mut self) -> SyscallResult {
        return if let Some(proc) = self.get_running() {
            if proc.get_pid() == 1 {
               self.set_panicd();
            }

            self.dec_number();
            self.recompute_quanta();
            self.set_running(None);
            SyscallResult::Success
        } else {
            SyscallResult::NoRunningProcess
        }
    }
}

pub trait SchedulerActions : ProcessManager {
    fn decide_sleep(&mut self, decision: SchedulingDecision) {
        if let SchedulingDecision::Sleep(time) = decision {
            self.set_slept_time(time.get());
        } else {
            self.set_slept_time(0);
        }
    }

    fn wakeup_myself(&mut self) {
        let time = self.get_slept_time();
        
        if time != 0 {
            self.make_timeskip(time);
            self.set_slept_time(0);
        }

        self.compute_sleeping_times();
        self.awake_processes();
    }

    fn is_panicd(&self) -> bool {
        if !self.get_panicd() {
            return false;
        }

        if self.has_ready_processes() ||
           self.has_sleeping_processes() || 
           self.has_waiting_processes() {
            return true;
        }

        if self.get_running().is_some() {
            return true;
        }

        return false;
    }

    fn is_blocked(&self) -> Option<SchedulingDecision> {
        if self.get_running().is_some() {
            return None;
        }

        if self.has_ready_processes() {
            return None;
        }

        if !self.has_sleeping_processes() && !self.has_waiting_processes() {
            return Some(SchedulingDecision::Done);
        }

        if !self.has_sleeping_processes() && self.has_waiting_processes() {
            return Some(SchedulingDecision::Deadlock);
        }

        if self.has_sleeping_processes() {
            let time = self.get_time_for_sleep();

            return Some(SchedulingDecision::Sleep(NonZeroUsize::new(time).unwrap()));
        }

        return None;
    }
}
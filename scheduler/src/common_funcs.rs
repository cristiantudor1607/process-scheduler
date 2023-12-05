use crate::{scheduler_info::SchedulerActions, scheduler, SyscallResult, GeneralProcess, ProcessControlBlock, Pid, Syscall, StopReason};

pub fn execute_fork(scheduler: &mut dyn SchedulerActions, remaining: usize,  priority: i8) -> SyscallResult {
    let reason = StopReason::Syscall { syscall: Syscall::Fork(priority), remaining };
    
    let new_pid: Pid;
    if let Some(mut pcb) = scheduler.get_running() {
        let time = pcb.get_interrupted(remaining, reason);
        scheduler.set_running(Some(pcb));

        scheduler.make_timeskip(time);

        new_pid = scheduler.fork(priority);
        scheduler.make_timeskip(1);
    } else {
        new_pid = scheduler.fork(priority);
        scheduler.make_timeskip(1);
    };

    SyscallResult::Pid(new_pid)
}

pub fn execute_exit(scheduler: &mut dyn SchedulerActions, remaining: usize) -> SyscallResult {
    if let Some(pcb) = scheduler.get_running() {
        scheduler.make_timeskip(pcb.get_payload() - remaining);
    }

    scheduler.kill_running()
}

pub fn execute_expired(scheduler: &mut dyn SchedulerActions) -> SyscallResult {
    let reason = StopReason::Expired;
    
    return if let Some(mut pcb) = scheduler.get_running() {
        let time = pcb.get_interrupted(0, reason);
        scheduler.make_timeskip(time);
        
        scheduler.set_running(None);
        SyscallResult::Success
    } else {
        SyscallResult::NoRunningProcess
    }
}

//! A scheduler library.
//!
//! This library provides the traits and structures necessary
//! to implement a process scheduler.
//!

use std::num::NonZeroUsize;

mod schedulers;

pub use schedulers::RoundRobinPCB;
pub use schedulers::RoundRobinScheduler;

use schedulers::PriorityQueuePCB;
use schedulers::PriorityQueueScheduler;

pub use schedulers::FairPCB;
pub use schedulers::FairScheduler;

mod scheduler;
pub use crate::scheduler::{
    Pid, Process, ProcessState, Scheduler, SchedulingDecision, StopReason, Syscall, SyscallResult,
};

mod common_types;
pub use crate::common_types::Timestamp;
pub use crate::common_types::Event;
pub use crate::common_types::Vruntime;

mod collector;
pub use crate::collector::Collector;
pub use crate::collector::collect_all;

mod process_control_block;
pub use crate::process_control_block::ProcessControlBlock;


/// Returns a structure that implements the `Scheduler` trait with a round robin scheduler policy
///
/// * `timeslice` - the time quanta that a process can run before it is preempted
/// * `minimum_remaining_timeslice` - when a process makes a system call, the scheduler
///                                 has to decode whether to schedule it again for the
///                                 remaining time of its quanta, or to schedule a new
///                                 process. The scheduler will schedule the process
///                                 again of the remaining quanta is greater or equal to
///                                 the `minimum_remaining_timeslice` value.
#[allow(unused_variables)]
pub fn round_robin(timeslice: NonZeroUsize, minimum_remaining_timeslice: usize) -> impl Scheduler {
    RoundRobinScheduler::new(timeslice, minimum_remaining_timeslice)
}

/// Returns a structure that implements the `Scheduler` trait with a priority queue scheduler policy
/// * `timeslice` - the time quanta that a process can run before it is preempted
/// * `minimum_remaining_timeslice` - when a process makes a system call, the scheduler
///                                 has to decode whether to schedule it again for the
///                                 remaining time of its quanta, or to schedule a new
///                                 process. The scheduler will schedule the process
///                                 again of the remaining quanta is greater or equal to
///                                 the `minimum_remaining_timeslice` value.
#[allow(unused_variables)]
pub fn priority_queue(
    timeslice: NonZeroUsize,
    minimum_remaining_timeslice: usize,
) -> impl Scheduler {
    PriorityQueueScheduler::new(timeslice, minimum_remaining_timeslice)
}

/// Returns a structure that implements the `Scheduler` trait with a simplified [cfs](https://opensource.com/article/19/2/fair-scheduling-linux) scheduler policy
/// * `cpu_time` - the total time units that the cpu has for an iteration, this is used to compute
///                    the `timeslice` of each process.
/// * `minimum_remaining_timeslice` - when a process makes a system call, the scheduler
///                                 has to decode whether to schedule it again for the
///                                 remaining time of its quanta, or to schedule a new
///                                 process. The scheduler will schedule the process
///                                 again of the remaining quanta is greater or equal to
///                                 the `minimum_remaining_timeslice` value.
#[allow(unused_variables)]
pub fn cfs(cpu_time: NonZeroUsize, minimum_remaining_timeslice: usize) -> impl Scheduler {
    FairScheduler::new(cpu_time, minimum_remaining_timeslice)
}

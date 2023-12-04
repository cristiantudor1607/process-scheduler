use std::ops::Add;

use crate::{Pid, ProcessControlBlock, schedulers::PriorityQueuePCB};

pub const MIN_PRIO: i8 = 0;
pub const MAX_PRIO: i8 = 5;

#[derive(Clone, Copy)]
pub struct Timestamp(usize);

impl Timestamp {
    /// Creates a new Timestamp object
    /// 
    /// * `time` - inital value of the Timestamp
    pub fn new(time: usize) -> Timestamp {
        Timestamp(time)
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

impl Add<usize> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: usize) -> Self::Output {
        Timestamp::new(self.0 + rhs)
    }
}

#[derive(Clone, Copy)]
pub struct Event(usize);

impl Event {
    /// Creates a new Event object
    /// 
    /// * `event` - the event identifier as usize
    pub fn new(event: usize) -> Event {
        Event(event)
    }

    /// Gets the event as a usize value
    pub fn get(&self) -> usize {
        self.0
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn ne(&self, other: &Self) -> bool {
        self.0 != other.0
    }
}

#[derive(Clone, Copy)]
pub struct Vruntime(usize);

impl Vruntime {
    pub fn new(vruntime: usize) -> Vruntime{
        Vruntime(vruntime)
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

impl PartialEq for Vruntime {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn ne(&self, other: &Self) -> bool {
        self.0 != other.0
    }
}

impl PartialOrd for Vruntime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}

impl Add<usize> for Vruntime {
    type Output = Vruntime;

    fn add(self, rhs: usize) -> Self::Output {
        Vruntime::new(self.0 + rhs)
    }
}

// pub trait SchedulerInfo {
//     fn get_next_pid(&self) -> Pid;
//     fn set_next_pid(&mut self, pid: Pid);

//     fn get_timestamp(&self) -> Timestamp;
//     fn set_timestamp(&mut self, timestamp: Timestamp);

//     fn is_round_robin(&self) -> bool;
//     fn is_prio_round_robin(&self) -> bool;
//     fn is_cfs(&self) -> bool;
// }

// pub fn spawn_process(scheduler: &mut dyn SchedulerInfo,
//     prio: i8,
//     timestamp: Timestamp) -> impl ProcessControlBlock {
    
//     let pid = scheduler.get_next_pid();
//     inc_next_pid(scheduler);

//     if scheduler.is_prio_round_robin() {
//         return PrioRoundRobinPCB::new(pid, prio, timestamp);
//     }

//     panic!("It should not be called");
// }

// pub fn inc_next_pid(scheduler: &mut dyn SchedulerInfo) {
//     let pid = scheduler.get_next_pid();
//     let new_pid = pid.add(1);
//     scheduler.set_next_pid(new_pid);
// }

// pub fn make_timeskip(scheduler: &mut dyn SchedulerInfo, time: usize) {
//     let timestamp = scheduler.get_timestamp();
//     let new_timestamp = timestamp.add(time);
//     scheduler.set_timestamp(new_timestamp);
// }



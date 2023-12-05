//! Implement the schedulers in this module
//!
//! You might want to create separate files
//! for each scheduler and export it here
//! like
//!
//! ```ignore
//! mod scheduler_name
//! pub use scheduler_name::SchedulerName;
//! ```
//!
mod round_robin_pcb;
pub use round_robin_pcb::RoundRobinPCB;

mod priority_queue_pcb;
pub use priority_queue_pcb::PriorityQueuePCB;

mod cfs_pcb;
pub use cfs_pcb::FairPCB;

mod round_robin;
pub use round_robin::RoundRobinScheduler;

mod priority_queue;
pub use priority_queue::PriorityQueueScheduler;

mod cfs;
pub use cfs::FairScheduler;

// mod CFS_pcb;
// pub use CFS_pcb::FairPCB;

// mod CFS;
// pub use CFS::FairScheduler;
// TODO import your schedulers here

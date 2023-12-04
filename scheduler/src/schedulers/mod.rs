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

// TODO delete this example
mod empty;
pub use empty::Empty;

mod priority_queue_pcb;
pub use priority_queue_pcb::PriorityQueuePCB;

mod round_robin;
pub use round_robin::RoundRobinScheduler;

mod priority_queue;
pub use priority_queue::PriorityQueueScheduler;

// TODO import your schedulers here

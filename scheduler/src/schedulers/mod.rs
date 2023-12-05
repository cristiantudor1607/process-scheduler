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

mod process_block;
pub use process_block::GeneralProcess;

mod round_robin;
pub use round_robin::RoundRobinScheduler;




use crate::scheduler::Process;
pub trait Collector {
    // Returns a list of running processes
    fn collect_running(&self) -> Vec<&dyn Process>;

    // Returns a list of ready processes
    fn collect_ready(&self) -> Vec<&dyn Process>;

    // Returns a list of sleeping processes
    fn collect_sleeping(&self) -> Vec<&dyn Process>;

    // Returns a list of waiting processes
    fn collect_waiting(&self) -> Vec<&dyn Process>;
}

pub fn collect_all(scheduler: &dyn Collector) -> Vec<&dyn Process> {
    let mut procs: Vec<&dyn Process> = Vec::new();

    for item in scheduler.collect_running() {
        procs.push(item);
    }

    for item in scheduler.collect_ready() {
        procs.push(item);
    }

    for item in scheduler.collect_sleeping() {
        procs.push(item);
    }

    for item in scheduler.collect_waiting() {
        procs.push(item);
    }

    procs
}
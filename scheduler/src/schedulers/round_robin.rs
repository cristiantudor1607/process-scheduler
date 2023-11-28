use std::{num::NonZeroUsize, ops::Add};
use crate::{scheduler::{Pid, ProcessState, Process, SchedulingDecision, StopReason, 
SyscallResult, Scheduler}, Syscall};

#[derive(Clone, Copy)]
pub struct RoundRobinPCB {
	pid: Pid,
	state: ProcessState,
	remaining_time: usize,
	execution_time: usize,
}

impl RoundRobinPCB {
	pub fn new(
		pid: Pid,
		state: ProcessState,
		remaining_time: usize,
		execution_time: usize) 
		-> RoundRobinPCB {
			RoundRobinPCB {
				pid,
				state,
				remaining_time,
				execution_time,
			 }
		}
}

impl Process for RoundRobinPCB {
	fn pid(&self) -> Pid {
		self.pid
	}

	fn state(&self) -> ProcessState {
		self.state
	}

	fn timings(&self) -> (usize, usize, usize) {
		let total = self.remaining_time + self.execution_time;
		(total, 0, self.execution_time)
	}

	fn priority(&self) -> i8 {
		0
	}

	fn extra(&self) -> String {
		String::new()
	}
}


pub struct RoundRobinScheduler {
	ready: Vec<Box<dyn Process>>,
	waiting: Vec<Box<dyn Process>>,
	next_pid: Pid,
	time_quanta: NonZeroUsize,
	min_timeslice: usize,
	current_process: Option<RoundRobinPCB>,
}

unsafe impl Send for RoundRobinScheduler {}


impl RoundRobinScheduler {
	pub fn new(timeslice: NonZeroUsize, minimum_timeslice: usize) -> RoundRobinScheduler{
		RoundRobinScheduler {
			ready: Vec::new(),
			waiting: Vec::new(),
			next_pid: Pid::new(1),
			time_quanta : timeslice,
			min_timeslice : minimum_timeslice,
			current_process: None,
		}
	}
}

impl Scheduler for RoundRobinScheduler {
	fn next(&mut self) -> SchedulingDecision {
		println!("NEXT");
		match &self.current_process {
			Some(pcb) => {
				SchedulingDecision::Run { pid: pcb.pid, 
					timeslice: self.time_quanta }
			},
			None => SchedulingDecision::Done
		}
	}

	fn stop(&mut self, reason: StopReason) -> SyscallResult {
		println!("STOP");

		match reason {
			StopReason::Syscall { syscall, remaining } => {
				println!("{:?}, {}", syscall, remaining);
				
				match syscall {
					Syscall::Fork(p) => {
						let new_process = RoundRobinPCB::new(
							self.next_pid,
							ProcessState::Ready,
							remaining,
							0,
						);
						self.next_pid = self.next_pid.add(1);
						self.ready.push(Box::new(new_process));
						self.current_process = Some(new_process);
						
						return SyscallResult::Pid(new_process.pid);
					},
					Syscall::Exit => {
						match self.current_process {
							None => return SyscallResult::NoRunningProcess,
							Some(pcb) => {
								self.ready.retain(|f| f.pid() != pcb.pid);
								return SyscallResult::Success;
							}
						}
					},
					_ => return SyscallResult::Success
				}
			},
			StopReason::Expired => {
				SyscallResult::Success
			}
		}
	}

	fn list(&mut self) -> Vec<&dyn Process> {
		println!("LIST");
		let mut result: Vec<&dyn Process> = Vec::new();
		result.extend(self.ready.iter().map(|pcb| pcb.as_ref()));
		result
	}
}
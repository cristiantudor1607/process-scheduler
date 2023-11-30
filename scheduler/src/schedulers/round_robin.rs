use std::{num::NonZeroUsize, collections::VecDeque, ops::Add};
use crate::{scheduler::{Pid, ProcessState, Process, SchedulingDecision, StopReason, 
SyscallResult, Scheduler}, Syscall};

#[derive(Clone, Copy)]
pub struct RoundRobinPCB {
	pid: Pid,
	state: ProcessState,
	total_time: usize,
	syscall_time: usize,
	exec_time: usize,
}

impl RoundRobinPCB {
	fn new(pid: Pid) -> RoundRobinPCB {
		RoundRobinPCB { 
			pid: pid,
			state: ProcessState::Ready,
			total_time: 0,
			syscall_time: 0,
			exec_time: 0
		 }
	}

	fn set_state(&mut self, new_state: ProcessState) {
		self.state = new_state;
	}

	fn add_execution_time(&mut self, time: usize) {
		self.exec_time += time;
		self.total_time += time;
	}

	fn add_sleeping_time(&mut self, time: usize) {
		self.total_time += time;
	}

	fn add_syscall_time(&mut self) {
		self.syscall_time += 1;
		self.total_time += 1;
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
		(self.total_time, self.syscall_time, self.exec_time)
	}

	fn priority(&self) -> i8 {
		0
	}

	fn extra(&self) -> String {
		String::new()
	}
}

pub struct RoundRobinScheduler {
	ready: VecDeque<RoundRobinPCB>,
	waiting: VecDeque<RoundRobinPCB>,
	sleeping: VecDeque<(RoundRobinPCB, usize)>,
	running: Option<RoundRobinPCB>,
	next_pid: Pid,
	quanta: NonZeroUsize,
	min_slice: usize,
	last_remaining_time: usize,
}

impl RoundRobinScheduler {
	pub fn new(timeslice: NonZeroUsize, minimum_remaining_timeslice: usize) -> RoundRobinScheduler {
		RoundRobinScheduler { 
			ready: VecDeque::new(), 
			waiting: VecDeque::new(),
			sleeping: VecDeque::new(),
			running: None, 
			next_pid: Pid::new(1), 
			quanta: timeslice, 
			min_slice: minimum_remaining_timeslice,
			last_remaining_time: 0,
		}
	}

	/// Returns a new RoundRobinPCB process, forked from the running process
	/// 
	/// * `remaining_time` - the number of units of time that the parent process didn't
	/// 				   used from it's allocated quanta
	/// 
	/// 
	fn fork(&mut self, remaining_time: usize) -> RoundRobinPCB {
		RoundRobinPCB::new(Pid::new(7))
	}

	fn inc_pid(&mut self) {
		self.next_pid = self.next_pid.add(1);
	}

	fn set_remaining(&mut self, remaining_time: usize) {
		self.last_remaining_time = remaining_time;
	}

	fn enqueue_running_process(&mut self) {
		match self.running {
			None => (),
			Some(pcb) => {
				self.running = None;
				self.ready.push_back(pcb);
			}
		}
	}

	fn get_running_pid(&mut self) -> Option<Pid> {
		match self.running {
			None => return None,
			Some(pcb) => return Some(pcb.pid), 
		}
	}

	fn dequeue_process(&mut self) {
		let proc = self.ready.pop_front();
		self.running = proc;
	}

	fn sleep_process(&mut self, proc: RoundRobinPCB, time: usize) {
		self.sleeping.push_back((proc, time));
		self.running = None;
	}

}

impl Scheduler for RoundRobinScheduler {
	fn stop(&mut self, reason: StopReason) -> SyscallResult {
		println!("Called stop");
		
		match reason {
			StopReason::Syscall { syscall, remaining } => {
				match syscall {
					Syscall::Fork(_) => {
						/* Set the remaining time from the last syscall , so the scheduler will know if the
						current running process can be replanified, or other process should be planified */
						self.set_remaining(remaining);

						/* Calculate the execution time before syscall fork happened */
						let elapsed_time = self.quanta.get() + remaining;
						
						/* Only at the beggining, when the process with pid 1 is spawned, there was no 
						running process when the syscall happened, but most of the time, there should certainly
						be a running process */
						match self.running {
							None => (),
							Some(mut pcb) => {
								// TODO: se poate sa trebuiasca si un minus 1 pe undeva
								pcb.add_execution_time(elapsed_time);
								
								/* For the running process, there was a syscall, so syscall time should be increased */
								pcb.add_syscall_time();
							},
						};


						let proc = RoundRobinPCB::new(self.next_pid);
						self.ready.push_back(proc);
						let pid = self.next_pid;

						self.inc_pid();

						return SyscallResult::Pid(pid);
					},
					Syscall::Sleep(time) => {
						todo!()
					},
					_ => todo!(),
				}
			},

			StopReason::Expired => {
				match self.running {
					Some(mut pcb) => {
						pcb.add_execution_time(self.quanta.get());
						pcb.set_state(ProcessState::Ready);
						
						self.enqueue_running_process();
						return SyscallResult::Success;
					},
					/* It is very possible to never reach this arm */
					None => return SyscallResult::NoRunningProcess,
				}
			},
		}
	}

	fn next(&mut self) -> SchedulingDecision {
		println!("Called next");
		
		match self.running {
			None => {
				self.dequeue_process();
				return SchedulingDecision::Run {
					pid: self.get_running_pid().unwrap(),
					timeslice: self.quanta }
			},
			Some(pcb) => {
				if self.last_remaining_time < self.min_slice {
					self.enqueue_running_process();
					self.dequeue_process();

					match self.running {
						Some(pcb) => {
							return SchedulingDecision::Run {
								pid: self.get_running_pid().unwrap(), 
								timeslice: self.quanta };
						},
						/* I think it won't reach this point */
						None => return SchedulingDecision::Done,
					}
				} else {
					return SchedulingDecision::Run {
						pid: self.get_running_pid().unwrap(),
						timeslice: NonZeroUsize::new(self.last_remaining_time).unwrap(),
					}
				}
			},
		};
	}

	fn list(&mut self) -> Vec<&dyn Process> {
		println!("Called list");

		let mut list: Vec<&dyn Process> = Vec::new();
		
		/* Add the running process */
		match &self.running {
			None => (),
			Some(pcb) => list.push(pcb),
		};

		/* Add the ready processes */
		for item in self.ready.iter() {
			list.push(item);
		};

		/* Add the waiting processes */
		for item in self.waiting.iter() {
			list.push(item);
		};

		for item in self.sleeping.iter() {
			list.push(&item.0);
		}

		return list;
	}
}

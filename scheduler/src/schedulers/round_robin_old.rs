use std::{num::NonZeroUsize, collections::VecDeque, ops::Add};
use crate::{scheduler::{Pid, ProcessState, Process, SchedulingDecision, StopReason, 
SyscallResult, Scheduler}, Syscall};

#[derive(Clone, Copy)]
pub struct RoundRobinPCB {
	pid: Pid,
	state: ProcessState,
	exec_time: usize,
	syscall_time: usize,
	sleep_time: usize,
}

impl RoundRobinPCB {
	fn new(pid: Pid) -> RoundRobinPCB {
		RoundRobinPCB { 
			pid,
			state: ProcessState::Ready,
			exec_time: 0,
			syscall_time: 0,
			sleep_time: 0,
		 }
	}

	/// Set the Process Control Block state to `new_state`
	fn set_state(&mut self, new_state: ProcessState) {
		self.state = new_state;
	}

	/// Add the `time` to the total time that the process spent executing
	fn add_execution_time(&mut self, time: usize) {
		self.exec_time += time;
	}

	/// Add the `time` to the total time that the process spent sleeping
	fn add_sleeping_time(&mut self, time: usize) {
		self.sleep_time += time;
	}

	/// Increase the number of syscalls sent by process
	fn add_syscall_time(&mut self) {
		self.syscall_time += 1;
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
		let total = self.exec_time + self.syscall_time + self.sleep_time;
		
		(total, self.syscall_time, self.exec_time)
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
	unused_time: usize,
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
			unused_time: 0,
		}
	}

	/// Returns the pid of the running process, if there is one, or None otherwise
	fn get_running_pid(&mut self) -> Option<Pid> {
		
		if let Some (pcb) = self.running {
			return Some(pcb.pid);
		} else {
			return  None;
		}
	}

	/// Makes the scheduler know about the time that the running process wasn't
	/// able to use from it's allocated quanta, because it was stopped by a
	/// syscall
	fn set_unused_time(&mut self, unused_time: usize) {
		self.unused_time = unused_time;
	}

	/// Increase the `next_pid` field of the RoundRobinScheduler
	/// 
	/// Method is called after a new process is spawned, so every process
	/// has it's unique pid
	/// 
	/// The `next_pid` field is strongly connected with the creation of 
	/// processes, so the actions should happen one after another
	fn inc_pid(&mut self) {
		self.next_pid = self.next_pid.add(1);
	}

	/// Crates a new Process Control Block for the scheduler, using the `next_pid`,
	/// and update the `next_pid` field of the scheduler, to mark as used the new spawned
	/// process pid
	fn spawn_process(&mut self) -> RoundRobinPCB {
		let new_proc = RoundRobinPCB::new(self.next_pid);
		self.inc_pid();

		new_proc
	}

	/// Returns a new RoundRobinPCB process, forked from the running process
	/// 
	/// It spawns a new process, and adds it to the queue
	/// 
	/// * `remaining_time` - the number of units of time that the parent process didn't
	/// 				   used from it's allocated time quanta
	fn fork(&mut self, remaining_time: usize) -> RoundRobinPCB {
		/* Set the time that the current running process of the scheduler didn't 
		used from it's time quanta */
		self.set_unused_time(remaining_time);

		/* Spawn a new process */
		let new_proc = self.spawn_process();
		self.ready.push_back(new_proc);

		new_proc
	}

	/// Enqueues the running process, if it exists, and returns Ok, otherwise does
	/// nothing and return an Err
	/// 
	/// The method add to total execution time the `used_time` send as parameter
	/// 
	/// * `used_time` - the units of time that the running process used from it's
	/// 			  quanta for execution
	fn enqueue_running_process(&mut self, used_time: usize) -> Result<(), ()> {
		return if let Some(mut pcb) = self.running {
			pcb.set_state(ProcessState::Ready);
			pcb.add_execution_time(used_time);
			self.ready.push_back(pcb);
			self.running = None;

			/* Make the caller know the operation was done */
			Ok(())
		} else {
			/* Make the caller know the operation did nothing */
			Err(())
		}
	}

	/// Makes running the next process waiting in the `ready` queue,
	/// or it sets the `running` process to None, if there isn't a single
	/// process in the `ready` queue
	fn dequeue_process(&mut self) {
		let proc = self.ready.pop_front();
		self.running = proc;

		if let Some(mut pcb) = self.running {
			pcb.set_state(ProcessState::Running);
		}
	}

	/// Returns the minimum time when a process will exit from sleep
	/// state, if there are processes in sleep, or None otherwise
	/// 
	fn get_sleep_time(&mut self) -> Option<usize> {
		/* If there are no process in sleep */
		if self.sleeping.is_empty() {
			return None;
		}

		/* Search for the minimum time that the scheduler should sleep */
		let mut minimum_time = usize::MAX;
		for item in self.sleeping.iter() {
			if item.1 < minimum_time {
				minimum_time = item.1;
			}
		}
	
		Some(minimum_time)
	}

	/// Updates the sleeping time for all processes in sleep state
	fn update_sleeping_time(&mut self, elapsed_time: usize) {
		
		for item in self.sleeping.iter_mut() {
			item.1 -= elapsed_time
		}
	}

	fn dequeue_sleeping(&mut self) {
		
	}

	/// Returns true if all the processes are waiting for a signal, or false
	/// otherwise
	fn is_deadlocked(&self) -> bool {
		if let Some(_) = self.running {
			return false;
		}
		
		if !self.ready.is_empty() {
			return false;
		}

		if !self.sleeping.is_empty() {
			return false;
		}

		if self.waiting.is_empty() {
			return false;
		}

		return true;
	}

}

impl Scheduler for RoundRobinScheduler {
	fn stop(&mut self, reason: StopReason) -> SyscallResult {
		println!("Called stop");
		
		if let StopReason::Syscall { syscall, remaining } = reason {
			if let Syscall::Fork(_) = syscall {
				/* Spawn a new process */
				let new_proc = self.fork(remaining);
				
				/* The first fork won't have a running process, but rest of the
				times it will have a parent process, so it should update the 
				syscall time of the parent */
				if let Some(mut pcb) = self.running {
					pcb.add_syscall_time();
				}

				return SyscallResult::Pid(new_proc.pid);
			}

			if let Syscall::Sleep(time) = syscall {
				todo!();
			}

			if let Syscall::Wait(event) = syscall {
				todo!();
			}

			if let Syscall::Signal(event) = syscall {
				todo!();
			}

			if let Syscall::Exit = syscall {
				todo!();
			}
		}

		/* If the reason isn't a syscall, then the time expired, so we should try to
		enqueue the running process */
		if let Ok(_) = self.enqueue_running_process(self.quanta.get()) {
			return SyscallResult::Success;
		}

		/* If the enqueue operation returned an error, then there was no running process */
		SyscallResult::NoRunningProcess

	}

	fn next(&mut self) -> SchedulingDecision {
		println!("Called next");
		
		let used_time = self.quanta.get() - self.unused_time;

		if self.unused_time < self.min_slice {
			/* Enqueue the running process and bring forward the next one */
			_ = self.enqueue_running_process(used_time);
			self.dequeue_process();
			
			/* Run the process dequeued, if there is one */
			if let Some(pcb) = self.running {
				return SchedulingDecision::Run {
					pid: pcb.pid,
					timeslice: self.quanta,
				};
			} else {
				/* At this point, there was no process in the ready queue, so it
				should search for processes from sleep state */
				if let Some(time) = self.get_sleep_time() {
					return SchedulingDecision::Sleep(NonZeroUsize::new(time).unwrap());
				}

				/* If there was no process in the sleep queue, we should check for a
				deadlock */
				if self.is_deadlocked() {
					return SchedulingDecision::Deadlock;
				}

				return SchedulingDecision::Done;
			}
		} else {
			/* Reschedule the current process  */
			return SchedulingDecision::Run {
				pid: self.get_running_pid().unwrap(),
				timeslice: NonZeroUsize::new(self.unused_time).unwrap(),
			};
		}

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

		/* Add the sleeping processes */
		for item in self.sleeping.iter() {
			list.push(&item.0);
		}

		return list;
	}
}

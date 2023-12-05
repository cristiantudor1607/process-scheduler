### Name : Tudor Cristian-Andrei
### Group: 321CA


# Process Scheduler

## Implementation Details

My implementation of the Process Scheduler leaves something to be desired, because I have many duplicates in code. I did it to work, in the simplest way possible.
> **Side note:** In the first place, I designed the scheduler to work. There is an early version on <span style="color:green">Refactored</span> branch, which wanted to be a implementation using the same *Process Control Block* for all schedulers. Each scheduler would have implemented some traits, and worked the same way, and the duplicate code from *next* and *stop* methods would have been replaced by functions that get a scheduler as parameter and return a *SyscallResult*. Unfortunatelly, there was no time for me to finish that version, but I will go back to it some day, because I like the concepts that I learned from this assignement.

All schedulers use the same principle. The *next* and *stop* functions were made side by side. The *stop* method takes all the cases possible and do something different for each of them. For the *next* method I had to treat 3 cases:

1. For processes that send a **Sleep** or **Wait** syscalls, or when a process is preempted because it gets an **StopReason::Expired**, it has to choose a new process from the available ones.
2. When a process is interrupted by other syscalls, and it can't continue running, a new process is choosen.
3. When a process is interrupted by other syscalls, and it can continue running.

For managing the timings, the schedulers use a **Timestamp**, which is updated at every action performed. The **Process Control Block** structures also have an **arrival_time** field, which indicates the timestamp when processes have been spawned. The **total_time** of the processes is computed using those 2.

When the process with **PID 1** is killed, a trigger is activated, named **panicd**, that is later used to detect if there is a situation of **Panic**.

There is also an additional field in schedulers design, **slept_time** which is set only if the scheduler has to sleep. When the scheduler is awoken, it is rested and the timestamp is updated using this extra-field.

For creating new processes, the **next_pid** field of the scheduler is used. It indicates what the name says. It is only incremented, never decremented, or set, so it respects the property that each process has it's unique id, and the pids aren't reused after a process is killed.

A scheduler can or cannot have a running process at some point in time. For that I used the **Option** enum provided by Rust Language. The running process is hold within the **running** field, as the name suggests. If there is no running process, it is set to **None**. This property is used everywhere in the program to determine if there is a process in execution.

**Side note** : Each syscall has the same pattern:
```rust
if let Syscall::Wait(event) = syscall {
    if let Some(mut pcb) = self.running {
        // Update the timings and the timestamp
        let time = pcb.get_interrupted(remaining, reason);
        self.make_timeskip(time);

        // Make something that has an effect on the scheduler internal state
        self.do_something_with_the_process(pcb, other_params);

        self.running = None;
        // or
        self.running = Some(pcb);
        return SyscallResult::Success;
    }
}
```
> Within the `do_something_with_the_process` method, it will always be a `self.make_timeskip(1)`, which signifies the time consumed by the syscall. It takes one unit of time.

When a process is being prepared to run, it is loaded with a payload, measured in time units (**time_payload** field). This is useful when a process has to continue running after being stopped by a syscall, because if it expires or it is stopped again, the scheduler will know the maximum time that the process could have run.

> When the process continues, it is re-launched like this:
```rust
return SchedulingDecision::Run {
    pid: proc.pid,
    timeslice: NonZeroUsize::new(proc.time_payload).unwrap(),
};
```
> and now, the payload serves as quanta.

## Details about Round Robin Scheduler
* The Round Robin Scheduler uses 3 "queues"[^1]. There is a ready queue, which holds the ready processes in order. The first process from **ready** will be planified next. The running process goes to the end of the queue, and waits for the rest to run. The **sleeping** queue holds tuples (**Process Control Block**, **timestamp when process was added to the queue**, **time that process has to sleep**), and the time is set to 0, only when the process finished it's sleeping time. The time set to 0 indicates that the process has to be awoken. There is the third queue, which holds tuples (**Process Control Block**, **Event**), and the process can leave the queue, only when there is a **Signal** sent with that event.

## Details about Priority Queue Scheduler
* It uses a **HashMap** instead of **VecDequeue**, for the ready processes. It contains entries like (**priority**, **ready queue for priority**). When a process is requested, it goes through each entry, starting from **MAX_PRIO** to **MIN_PRIO** and it stops when it finds the first process in a queue.
* When a process is added to the queue, it takes the value of the process priority from the **ready** HashMap, and put the process in the value returned, which is a VecDequeue. If the priority isn't in the HashMap, it creates a new VecDequeue and adds it to the map.
* PCBs of this scheduler have a **priority_at_born** field, that stores the priority that the process gained when it was forked, and it cannot be modified. Only the **priority** field is increased, but it won't surpass the **priority_at_born**, or decreased, but it won't go below 0.

## Details about CFS
* Instead of a **VecDequeue** or a **HashMap**, it uses a simple **Vec**, for ready processes, because this time, the scheduler has to search for the minimum **vruntime** and there is no predetermined order.
* The next process to be run is choosen using a method that searches through the **ready** Vec, and finds the process with the minimum **vruntime**. In case of equality, it takes the one that was created first, the one with the smallest **PID**.
* There is an additional **proc_number**, which stores the number of processes. It's only usage is to recompute the quanta, when a new process is being created, killed, put to sleep, or blocked by an event.

## Timestamp, Event and Vruntime
* Basically they're a packed `usize`
* I was inspired by `Pid` struct
* Motivation: For vecs that contains tuples, it's more expressive to have (**PCB**, **Specific Type**, ...), and it limits the usage of that type. For example, it prevents comparing a usize with a Timestamp, or adding or subtracting directly those 2.

## Collector trait
* It's the only concept that I managed to keep from Refactored version. Each scheduler has to implement this trait to generate directly a lists of all processes, through a function, `collect_all`.
> **Side note:** I also kept **Process Control Block** trait, but it doesn't have the same practical usage as **Collector**, because it was designed to act like a prerquiste for a bigger, more useful trait. It just defines the same methods for all PCBs, but it is limited to that.

## Improvements
* More documentation
> **Side note** : I managed to add documentation for **Round Robin**, because I wrote it as I designed the Round Robin.
* Refactorization: The project overall should be refactored, like I tried on <span style="color:green">Refactored</span> branch, using more traits, and a common structure for the **Process Control Block**.

[^1]: I said "queues" because it uses only one **VecDequeue** and 2 **Vec** that acts like a queue.


use std::ops::Add;

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


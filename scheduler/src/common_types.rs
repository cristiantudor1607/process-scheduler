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

    /// Makes a skip in time
    pub fn timeskip(&mut self, time: usize) {
        self.0 += time;
    }
}

pub struct Event(usize);

impl Event {
    /// Creates a new Event object
    /// 
    /// * `event` - the event identifier as usize
    pub fn new(event: usize) -> Event {
        Event(event)
    }
}

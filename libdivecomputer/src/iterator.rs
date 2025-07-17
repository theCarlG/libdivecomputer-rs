use std::sync::mpsc;

/// Iterator for downloaded dives with blocking and non-blocking next
pub struct DcIterator<T> {
    receiver: mpsc::Receiver<T>,
    finished: bool,
}

impl<T> DcIterator<T> {
    pub fn new(receiver: mpsc::Receiver<T>) -> Self {
        Self {
            receiver,
            finished: false,
        }
    }

    /// Try to get the next dive without blocking
    /// Returns None if no dive is immediately available
    pub fn try_next(&mut self) -> Option<T> {
        if self.finished {
            return None;
        }

        match self.receiver.try_recv() {
            Ok(dive) => Some(dive),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.finished = true;
                None
            }
        }
    }

    /// Check if the iterator is finished
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

impl<T> Iterator for DcIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        match self.receiver.recv() {
            Ok(dive) => Some(dive),
            Err(_) => {
                self.finished = true;
                None
            }
        }
    }
}

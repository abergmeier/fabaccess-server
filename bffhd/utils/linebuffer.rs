use std::ops::{Deref, DerefMut};

pub struct LineBuffer {
    buffer: Vec<u8>,
    valid: usize,
}

impl LineBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            valid: 0,
        }
    }

    /// Resize the internal Vec so that buffer.len() == buffer.capacity()
    fn resize(&mut self) {
        // SAFETY: Whatever is in memory is always valid as u8.
        unsafe { self.buffer.set_len(self.buffer.capacity()) }
    }

    /// Get an (initialized but empty) writeable buffer of at least `atleast` bytes
    pub fn get_mut_write(&mut self, atleast: usize) -> &mut [u8] {
        let avail = self.buffer.len() - self.valid;
        if avail < atleast {
            self.buffer.reserve(atleast - avail);
            self.resize()
        }
        &mut self.buffer[self.valid..]
    }

    pub fn advance_valid(&mut self, amount: usize) {
        self.valid += amount
    }

    /// Mark `amount` bytes as 'consumed'
    ///
    /// This will move any remaining data to the start of the buffer for future processing
    pub fn consume(&mut self, amount: usize) {
        assert!(amount <= self.valid);

        if amount < self.valid {
            self.buffer.copy_within(amount..self.valid, 0);
        }
        self.valid -= amount;
    }
}

impl Deref for LineBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.buffer[0..self.valid]
    }
}
impl DerefMut for LineBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer[0..self.valid]
    }
}

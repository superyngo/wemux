//! Lock-free ring buffer for audio data

use std::sync::atomic::{AtomicUsize, Ordering};

/// Lock-free single-producer single-consumer ring buffer
///
/// Used to decouple the capture thread from render threads.
/// Each renderer should have its own read position tracked externally.
pub struct RingBuffer {
    buffer: Box<[u8]>,
    capacity: usize,
    write_pos: AtomicUsize,
    /// Mask for fast modulo operation (only works when capacity is power of 2)
    mask: usize,
}

impl RingBuffer {
    /// Create a new ring buffer with the specified capacity
    ///
    /// Capacity will be rounded up to the next power of 2 for efficiency
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.next_power_of_two();
        let mask = capacity - 1;

        Self {
            buffer: vec![0u8; capacity].into_boxed_slice(),
            capacity,
            write_pos: AtomicUsize::new(0),
            mask,
        }
    }

    /// Get the buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the current write position
    pub fn write_position(&self) -> usize {
        self.write_pos.load(Ordering::Acquire)
    }

    /// Write data to the buffer (single producer)
    ///
    /// Returns the number of bytes written.
    /// This always succeeds - old data will be overwritten if buffer is full.
    pub fn write(&self, data: &[u8]) -> usize {
        let write_pos = self.write_pos.load(Ordering::Relaxed);

        for (i, &byte) in data.iter().enumerate() {
            let pos = (write_pos + i) & self.mask;
            // SAFETY: We're the only writer and pos is always valid
            unsafe {
                let ptr = self.buffer.as_ptr() as *mut u8;
                std::ptr::write_volatile(ptr.add(pos), byte);
            }
        }

        // Update write position
        let new_pos = write_pos.wrapping_add(data.len());
        self.write_pos.store(new_pos, Ordering::Release);

        data.len()
    }

    /// Read data from the buffer at the given read position
    ///
    /// Returns the number of bytes read and updates the read position.
    /// The reader is responsible for tracking their own read position.
    pub fn read(&self, buf: &mut [u8], read_pos: &mut usize) -> usize {
        let write_pos = self.write_pos.load(Ordering::Acquire);
        let available = write_pos.wrapping_sub(*read_pos);

        // Don't read more than available or more than buffer size
        let to_read = buf.len().min(available).min(self.capacity);

        #[allow(clippy::needless_range_loop)]
        for i in 0..to_read {
            let pos = (*read_pos + i) & self.mask;
            // SAFETY: pos is always valid due to mask
            unsafe {
                let ptr = self.buffer.as_ptr();
                buf[i] = std::ptr::read_volatile(ptr.add(pos));
            }
        }

        *read_pos = read_pos.wrapping_add(to_read);
        to_read
    }

    /// Calculate available bytes to read from a given read position
    pub fn available(&self, read_pos: usize) -> usize {
        let write_pos = self.write_pos.load(Ordering::Acquire);
        let available = write_pos.wrapping_sub(read_pos);
        available.min(self.capacity)
    }

    /// Check if reader is lagging behind (data was overwritten)
    pub fn is_lagging(&self, read_pos: usize) -> bool {
        let write_pos = self.write_pos.load(Ordering::Acquire);
        write_pos.wrapping_sub(read_pos) > self.capacity
    }

    /// Reset reader position to current write position (catch up)
    pub fn catch_up(&self, read_pos: &mut usize) {
        *read_pos = self.write_pos.load(Ordering::Acquire);
    }
}

/// Per-renderer read state for the shared ring buffer
pub struct ReaderState {
    read_pos: usize,
}

impl ReaderState {
    /// Create a new reader state starting from the current write position
    pub fn new(buffer: &RingBuffer) -> Self {
        Self {
            read_pos: buffer.write_position(),
        }
    }

    /// Read data from the shared buffer
    pub fn read(&mut self, buffer: &RingBuffer, buf: &mut [u8]) -> usize {
        buffer.read(buf, &mut self.read_pos)
    }

    /// Get available bytes to read
    pub fn available(&self, buffer: &RingBuffer) -> usize {
        buffer.available(self.read_pos)
    }

    /// Check if this reader is lagging
    pub fn is_lagging(&self, buffer: &RingBuffer) -> bool {
        buffer.is_lagging(self.read_pos)
    }

    /// Catch up to current write position (skip data)
    pub fn catch_up(&mut self, buffer: &RingBuffer) {
        buffer.catch_up(&mut self.read_pos)
    }

    /// Get current read position
    #[allow(dead_code)]
    pub fn position(&self) -> usize {
        self.read_pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_write_read() {
        let buffer = RingBuffer::new(1024);
        let mut reader = ReaderState::new(&buffer);

        let data = [1u8, 2, 3, 4, 5];
        buffer.write(&data);

        let mut read_buf = [0u8; 5];
        let read = reader.read(&buffer, &mut read_buf);

        assert_eq!(read, 5);
        assert_eq!(read_buf, data);
    }

    #[test]
    fn test_wrap_around() {
        let buffer = RingBuffer::new(8); // Will be 8 (already power of 2)
        let mut reader = ReaderState::new(&buffer);

        // Write 6 bytes
        buffer.write(&[1, 2, 3, 4, 5, 6]);

        // Read 4
        let mut read_buf = [0u8; 4];
        reader.read(&buffer, &mut read_buf);
        assert_eq!(read_buf, [1, 2, 3, 4]);

        // Write 4 more (should wrap)
        buffer.write(&[7, 8, 9, 10]);

        // Read remaining
        let mut read_buf = [0u8; 6];
        let read = reader.read(&buffer, &mut read_buf);
        assert_eq!(read, 6);
        assert_eq!(&read_buf[..6], &[5, 6, 7, 8, 9, 10]);
    }
}

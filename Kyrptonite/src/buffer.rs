//! A buffer for reading data from the network.
//!
//! The `ReadBuffer` is a buffer of bytes similar to a first-in, first-out queue.
//! It is filled by reading from a stream supporting `Read` and is then
//! accessible as a cursor for reading bytes.

use std::io::{Read, Result as IoResult};
use bytes::Buf;

/// A FIFO buffer for reading packets from the network.
/// Since Kryptonite's `ReadBuffer` is a Ring-Buffer meant for low-latency
/// Frame Processing, there are some things to keep in mind.
/// 1. After every `read_from` call, `take_ref` should be used [there can be any number of `take_ref` calls after `read_from`
/// but not vice-versa for the way how `ReadBuffer` is implemented].
/// Although this is how websockets are used i.e. we first read bytes from the stream and then 
/// process the frames we recieved in those bytes. DO NOT `read_from` multiple times thinking it will
/// accumulate some data and then read using `take_ref`. If that is the use-case, it is not low-latency usage
/// which Kryptonite is meant for. But still if that is required, simply wait (`thread::sleep`) a bit before 
/// `read_from` to get more frames in the `TCPStream` but waiting too long might make the server drop your connection
/// for the websocket stream getting congested from unread websocket frames.
/// 2. Since, at some point the ring buffer will again start over-writing old data make sure to be fast with processing
/// the frames. ReadBuffer provides reference to the frame as a slice (fat-pointer) to the respective position in the
/// ring buffer making it fast and zero-copy.
#[derive(Debug)]
pub struct ReadBuffer<const CHUNK_SIZE: usize> {
    /// the read cursor
    pub read_cursor: usize,
    /// the write cursor
    pub write_cursor: usize,
    /// the end of the last message in the buffer
    /// since we have wrapping behaviour, it might happen that 
    /// the size of the buffer is 128 and the last valid message ended at 100.
    /// the next message was 30 bytes long so we wrapped around and now the first
    ///  30 bytes are occupied by the latest message. `end` will be 101 in this case.
    pub end: usize,
    /// Sometimes the bytes of one frame can be split across multiple `read_from`
    /// calls, which can only be known after we have read the header of some bytes we already have
    /// the `curr_frame` will indicate the starting index of the latest frame. It helps tackle the
    /// half-read frame. It is not useful when the `write_cursor` is near the start or middle
    /// of the ring buffer `chunk`. But when near the end, it might need to migrate the current read bytes and 
    /// previously read bytes of the same frame to the start. It needs to be set from the frame parser which
    /// knows where a frame starts.
    pub curr_frame: usize,
    /// reads the stream in chunks
    pub chunk: Box<[u8; CHUNK_SIZE]>
}

impl<const CHUNK_SIZE: usize> ReadBuffer<CHUNK_SIZE> {
    /// Create a new empty input buffer.
    pub fn new() -> Self {
        Self::with_capacity(CHUNK_SIZE)
    }

    /// Create a new empty input buffer with a given `capacity`.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_partially_read(Vec::with_capacity(capacity))
    }

    /// Create a input buffer filled with previously read data.
    pub fn from_partially_read(part: Vec<u8>) -> Self {
        let n = part.len();
        // create a new buffer
        let mut chunk = Box::new([0u8; CHUNK_SIZE]);
        chunk[..n].copy_from_slice(&part);
        Self { read_cursor: 0, write_cursor: n, end: n, curr_frame: 0, chunk: chunk }
    }

    /// Set current `read_cursor` as the start of a frame.
    pub fn set_curr_frame(&mut self) {
        self.curr_frame = self.read_cursor;
    }

    /// Takes the slice of length `cnt` from `read_cursor` (including `read_cursor`)
    /// Acts as if we are consuming the buffer so `read_cursor` is updated
    /// If `read_cursor` reaches `end` while consuming, this brings back `read_cursor` to 0 
    /// if we have wrapped around and written some message at the start of the buffer.
    /// Does not support wrapping i.e. if your `read_cursor + end` goes past `end` it panics!
    /// Instead you can call this twice with proper `cnt` to get the tail and head of the buffer respectively.
    pub fn take_ref(&mut self, cnt: usize) -> &[u8] {
        // the point till which we will read
        let cutoff = self.read_cursor + cnt;
        // if this is past the `end` then panic!
        if cutoff > self.end {
            println!("{} {} {} {} {} {}", CHUNK_SIZE, self.read_cursor, self.write_cursor, self.end, self.curr_frame, cnt);
            panic!("Reading past the last message end in the buffer before wrapping is not allowed"); 
        }
        if cutoff == self.end && self.write_cursor < self.end {
            self.read_cursor = 0;
            // once we have consumed the last message in the buffer before wrapping.
            // `end` is now the `write_cursor` of the last message
            self.end = self.write_cursor;
        } else {
            self.read_cursor = cutoff;
        }
        &self.chunk[(cutoff - cnt)..cutoff]
    }

    /// Consume (Kind of) the `ReadBuffer` and get data
    pub fn into_vec(&mut self) -> Vec<u8> {
        if self.read_cursor < self.write_cursor {
            // update the `read_cursor` to `write_cursor` position
            // therefore making it look like consuming the data
            let prev_read_cursor = self.read_cursor;
            self.read_cursor = self.write_cursor;
            // Now we can safely return the internal container
            return self.chunk[prev_read_cursor..self.write_cursor].to_vec();
        } else {
            // if due to wrap around the write cursor is behind the read cursor.
            // we return all the valid frames from `read_cursor` to `end` and 
            // also all the valid frames from `0` to `write_cursor`
            let mut v = self.chunk[self.read_cursor..self.end].to_vec();
            v.extend_from_slice(&self.chunk[..self.write_cursor]);
            return v;
        }
    }

    /// The websocket frames can be a continue frame which is a continuation of the payload
    /// of the last sent frame. But while reading from the webscoket stream into our ring buffer
    /// we also read the header of the continuation frame which means that the previous frame's
    /// payload and its continuation are separated by the header bytes which makes the payload
    /// non-contiguous. So, whenever we receive a continue frame we need to shift the payload to make
    /// it continous with the previous payload. `adj_size` is the number of bytes between the previous
    /// payload's end and current payload's start. The caller of this must make sure that the continue frame
    /// does not use `set_curr_frame` o.w. the first frame might be at the end of `chunk` whereas the continuation
    /// frame is at the start of the `chunk` making it not possible to adjust.
    /// The current `read_cursor` is just after the header of the current frame because after parsing it
    /// has known that the current frame is a continue frame from the OPCODE
    pub fn adjust_continue_payload(&mut self, adj_size: usize, curr_payload_length: usize) {
        // store the current `read_cursor`
        let prev_read_cursor = self.read_cursor;
        // the position just after the end of the current frame
        let cutoff = self.read_cursor + curr_payload_length;
        // update the `read_cursor` --> Since a continue frame will always come after the first text frame in the continguous block
        // subtracting the `adj_size` won't make this counter go -ve. (Responsibility of the caller to call it correctly)
        self.read_cursor -= adj_size;
        self.chunk.copy_within(prev_read_cursor..cutoff, self.read_cursor);
    }

    /// Read next portion of data from the given input stream.
    pub fn read_from<S: Read>(&mut self, stream: &mut S) -> IoResult<usize> {
        // in case we need to wrap and read again we use size_again for the second read
        let mut size_again = 0usize;
        // read the stream data into our buffer
        let size = stream.read(&mut self.chunk[self.write_cursor..])?;
        // update the `write_cursor` and accomodate any possible wrap arounds
        self.write_cursor = (self.write_cursor + size) & (CHUNK_SIZE-1); // equivalent to % CHUNK_SIZE; only if CHUNK_SIZE = 2^p form
        // if `write cursor` is ahead of `end` will be updated
        if self.end < self.write_cursor { self.end = self.write_cursor; }
        // if after the write on the `chunk` tail, we exhaust the space to write
        // we wrap around then we copy the tail to the start and write again
        if self.write_cursor == 0 {
            // if the data wraps around then the end is updated to be the index just after the previous frame (i.e. start of the current frame)
            self.end = self.curr_frame;
            self.chunk.copy_within(self.curr_frame.., 0);
            // since the new position from where the write cursor starts writing is the size of the current frame read till now
            self.write_cursor = CHUNK_SIZE - self.curr_frame;
            size_again = stream.read(&mut self.chunk[self.write_cursor..])?;
            self.write_cursor = (self.write_cursor + size_again) & (CHUNK_SIZE-1); // equivalent to % CHUNK_SIZE; only if CHUNK_SIZE = 2^p form
            if self.write_cursor == 0 { // TODO: Some fix?
                println!("{} {} {} {} {}", CHUNK_SIZE, self.read_cursor, self.write_cursor, self.end, self.curr_frame);
                panic!("The stream contains more than {} bytes and cannot be read in the buffer in one go!", CHUNK_SIZE-1)
            }
            if self.read_cursor < self.write_cursor {
                // if we haven't read (let's call it consume) the ring buffer for a long time
                // and the `write_cursor` has wrapped around and overtaken the `read_cursor`
                // some portion of the data which could have been consumed by the `read_cursor`
                // has been overwritten and there is no way to recover it now. So, simply
                // reset the read cursor to the start so it can start consuming the latest message.
                self.read_cursor = 0
            }
            // For the frame that is read just before wrapping around, we have the curr_frame as the position
            // of the start of the payload index. Now, the next frame which we read can be either `Text`/`Continue`.
            // We don't know which will happen for `Text`, we can move the `read_cursor` to the start at `0`.
            // But for `Continue`, we need to keep the previous frame and the currently read frame together but if our
            // read_cursor >= curr_frame (the start of the starting `Text` frame after which `Continue` came). We have
            // already read some portion and thus the proper `read_cursor` position is `read_cursor - curr_frame` after wrapping.
            // We here account for the worst case that the frame at which wrapping around happens is a `Continue` Frame.
            // After wrapping around read_cursor then there is no use of `end` anymore since we have already consumed
            // the last message. So update the `end`.
            if self.read_cursor >= self.end { 
                self.read_cursor = self.read_cursor - self.curr_frame;
                self.end = self.write_cursor;
            }
            // if `write cursor` is ahead of `end` will be updated
            if self.end < self.write_cursor { self.end = self.write_cursor; }
            // update current frame to the starting position
            self.curr_frame = 0;
        }
        Ok(size + size_again)
    }

}

impl<const CHUNK_SIZE: usize> Buf for ReadBuffer<CHUNK_SIZE> {
    fn remaining(&self) -> usize {
        if self.write_cursor >= self.read_cursor {
            self.write_cursor - self.read_cursor
        } else {
            println!(" --> {} {} {} {} {}", CHUNK_SIZE, self.end, self.read_cursor, self.write_cursor, self.curr_frame);
            (self.end - self.read_cursor) + self.write_cursor
        }
    }

    fn chunk(&self) -> &[u8] {
        // Assumes that `read_cursor` is always actively reading just after we `read_from`
        // the stream. o.w. it might fail if `read_cursor` is near the tail and `write_cursor` near the start
        &self.chunk[self.read_cursor..self.write_cursor]
    }

    fn advance(&mut self, cnt: usize) {
        self.take_ref(cnt);
    }
}

impl<const CHUNK_SIZE: usize> Default for ReadBuffer<CHUNK_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use super::*;

    #[test]
    fn partially_initialize() {
        let input = b"Hello World!".to_vec();
        let buffer = ReadBuffer::<4096>::from_partially_read(input);
        assert_eq!(&buffer.chunk[buffer.read_cursor..buffer.write_cursor], "Hello World!".as_bytes());
    }

    #[test]
    fn simple_reading() {
        let mut input = Cursor::new(b"Hello World!".to_vec());
        let mut buffer = ReadBuffer::<4096>::new();
        let size = buffer.read_from(&mut input).unwrap();
        assert_eq!(size, 12);
        assert_eq!(&buffer.chunk[buffer.read_cursor..buffer.write_cursor], "Hello World!".as_bytes());
    }

    #[test]
    fn slide_reading() {
        let mut input = Cursor::new(b"Hello World!".to_vec());
        let mut buffer = ReadBuffer::<4096>::new();
        let size = buffer.read_from(&mut input).unwrap();
        assert_eq!(size, 12);
        assert_eq!(&buffer.chunk[buffer.read_cursor..buffer.write_cursor], "Hello World!".as_bytes());
        let read_cursor = buffer.read_cursor + buffer.write_cursor;

        let mut input = Cursor::new(b"Hello World!".to_vec());
        let size = buffer.read_from(&mut input).unwrap();
        assert_eq!(size, 12);
        assert_eq!(&buffer.chunk[read_cursor..buffer.write_cursor], "Hello World!".as_bytes());
    }

    #[test]
    fn wrap_reading() {
        let mut input = Cursor::new(b"Hello World!".to_vec());
        let mut buffer = ReadBuffer::<16>::new();
        let size = buffer.read_from(&mut input).unwrap();
        assert_eq!(size, 12);
        assert_eq!(buffer.take_ref(12), "Hello World!".as_bytes());

        buffer.set_curr_frame();

        let mut input = Cursor::new(b"Hello World!".to_vec());
        let size = buffer.read_from(&mut input).unwrap();
        assert_eq!(size, 12);
        assert_eq!(buffer.take_ref(12), "Hello World!".as_bytes());
    }

    #[test]
    #[should_panic(expected = "The stream contains more")]
    fn overflow_reading() {
        let mut input = Cursor::new(b"Hello World!".to_vec());
        let mut buffer = ReadBuffer::<8>::new();
        let _ = buffer.read_from(&mut input).unwrap();
    }

    #[test]
    fn two_messages() {
        let mut input = Cursor::new(b"Hello World!".to_vec());
        let mut buffer = ReadBuffer::<32>::new();
        let _ = buffer.read_from(&mut input).unwrap();

        let mut input = Cursor::new(b"Hello World!".to_vec());
        let _ = buffer.read_from(&mut input).unwrap();

        assert_eq!(buffer.into_vec(), b"Hello World!Hello World!".to_vec());
    }

    #[test]
    fn two_wrapping_messages() {
        let mut input = Cursor::new(b"Hello World!".to_vec());
        let mut buffer = ReadBuffer::<16>::new();
        let _ = buffer.read_from(&mut input).unwrap();

        buffer.advance(12);
        buffer.set_curr_frame();

        let mut input = Cursor::new(b"World!".to_vec());
        let _ = buffer.read_from(&mut input).unwrap();

        assert_eq!(buffer.into_vec(), b"World!".to_vec());
    }

    #[test]
    fn two_wrap_distinct_messages() {
        let mut input = Cursor::new(b"Hello World!".to_vec());
        let mut buffer = ReadBuffer::<16>::new();
        let _ = buffer.read_from(&mut input).unwrap();

        assert_eq!(buffer.into_vec(), b"Hello World!".to_vec());

        buffer.set_curr_frame();

        let mut input = Cursor::new(b"Hi".to_vec());
        let _ = buffer.read_from(&mut input).unwrap();

        buffer.set_curr_frame();

        let mut input = Cursor::new(b"Hello".to_vec());
        let _ = buffer.read_from(&mut input).unwrap();

        assert_eq!(buffer.into_vec(), b"HiHello".to_vec());
    }

    #[test]
    fn taking_ref() {
        let mut input = Cursor::new(b"Hello World!".to_vec());
        let mut buffer = ReadBuffer::<16>::new();
        let _ = buffer.read_from(&mut input).unwrap();

        assert_eq!(buffer.end, 12);
        assert_eq!(buffer.write_cursor, 12);
        assert_eq!(buffer.take_ref(5), b"Hello".to_vec());
        assert_eq!(buffer.read_cursor, 5);
        assert_eq!(buffer.take_ref(7), b" World!".to_vec());
        assert_eq!(buffer.read_cursor, 12);
        assert_eq!(buffer.end, 12);

        buffer.set_curr_frame();
        
        let mut input = Cursor::new(b"Hello".to_vec());
        let _ = buffer.read_from(&mut input).unwrap();

        assert_eq!(buffer.read_cursor, 0);
        assert_eq!(buffer.end, 5);
        assert_eq!(buffer.take_ref(5), b"Hello".to_vec());
    }

    #[test]
    fn adjust_continue_frame() {
        let mut input = Cursor::new(b"Hello World!".to_vec());
        let mut buffer = ReadBuffer::<32>::new();
        let _ = buffer.read_from(&mut input).unwrap();

        assert_eq!(buffer.take_ref(12), b"Hello World!".to_vec());
        
        let mut input = Cursor::new(b"<cont-header>Hello".to_vec());
        let _ = buffer.read_from(&mut input).unwrap();

        assert_eq!(buffer.take_ref(13), b"<cont-header>".to_vec());
        buffer.adjust_continue_payload(13, 5);
        assert_eq!(buffer.take_ref(5), b"Hello".to_vec());
        assert_eq!(buffer.chunk[0..17], b"Hello World!Hello".to_vec());
        assert_eq!(buffer.write_cursor, 30);
        assert_eq!(buffer.read_cursor, 17);
    }

    // #[test]
    // fn reading_in_chunks() {
    //     let mut inp = Cursor::new(b"Hello World!".to_vec());
    //     let mut buf = ReadBuffer::<4>::new();

    //     let size = buf.read_from(&mut inp).unwrap();
    //     assert_eq!(size, 4);
    //     assert_eq!(buf.chunk(), b"Hell");

    //     buf.advance(2);
    //     assert_eq!(buf.chunk(), b"ll");
    //     assert_eq!(buf.storage.get_mut(), b"Hell");

    //     let size = buf.read_from(&mut inp).unwrap();
    //     assert_eq!(size, 4);
    //     assert_eq!(buf.chunk(), b"llo Wo");
    //     assert_eq!(buf.storage.get_mut(), b"llo Wo");

    //     let size = buf.read_from(&mut inp).unwrap();
    //     assert_eq!(size, 4);
    //     assert_eq!(buf.chunk(), b"llo World!");
    // }
}

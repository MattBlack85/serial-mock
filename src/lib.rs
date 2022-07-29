use std::collections::VecDeque;
use std::io::{Error, ErrorKind};

pub struct MockableSerialBuilder {}

impl MockableSerialBuilder {
    pub fn new(
        address: &str,
        baud: u32,
        stop_byte: u8,
        read_n_bytes: u32,
        initial_response_data: Option<VecDeque<Vec<u8>>>,
    ) -> MockableSerial {
        let mut m = MockableSerial::new(address, baud, stop_byte, read_n_bytes);

        if let Some(r_data) = initial_response_data {
            for s in r_data.iter() {
                m.add_response(s);
            }
        }

        m
    }
}

pub struct MockableSerial {
    address: String,
    baud: u32,
    actual_success: bool,
    actual_response: Vec<u8>,
    response_queue: VecDeque<Vec<u8>>,
    success_queue: VecDeque<(bool, ErrorKind)>,
    stop_byte: u8,
    read_n_bytes: u32,
    last_read_index: usize,
}

pub trait SerialMock {
    fn new(address: &str, baud: u32, stop_byte: u8, read_n_bytes: u32) -> Self;
    fn open_native(&self) -> Self;
    fn write(&self, _b: &Vec<u8>) -> Result<(), std::io::Error>;
    fn read(&mut self, buff: &mut [u8]) -> Result<(), std::io::Error>;
    fn add_response(&mut self, r: &[u8]);
}

impl SerialMock for MockableSerial {
    fn new(address: &str, baud: u32, stop_byte: u8, read_n_bytes: u32) -> Self {
        Self {
            address: address.to_string(),
            baud,
            stop_byte,
            read_n_bytes,
            actual_success: true,
            actual_response: Vec::new(),
            response_queue: VecDeque::new(),
            last_read_index: 0,
            success_queue: VecDeque::new(),
        }
    }

    fn open_native(&self) -> Self {
        Self {
            address: self.address.clone(),
            baud: self.baud,
            stop_byte: self.stop_byte,
            read_n_bytes: self.read_n_bytes,
            actual_success: self.actual_success.clone(),
            actual_response: self.actual_response.clone(),
            response_queue: self.response_queue.clone(),
            last_read_index: self.last_read_index,
            success_queue: self.success_queue.clone(),
        }
    }

    fn write(&self, _b: &Vec<u8>) -> Result<(), std::io::Error> {
        Ok(())
    }

    fn read(&mut self, buff: &mut [u8]) -> Result<(), std::io::Error> {
        // Fetch a new item from the queue if there is nothing to read
        {
            if self.actual_response.is_empty() && !self.response_queue.is_empty() {
                self.actual_response
                    .append(&mut self.response_queue.pop_front().unwrap());
            }
        }

        let v = *self.actual_response.get(self.last_read_index).unwrap();
        buff[0] = v;

        if v == self.stop_byte {
            self.last_read_index = 0;
            self.actual_response.clear();
        } else {
            self.last_read_index += 1;
        }

        if self.actual_success {
            return Ok(());
        } else {
            return Err(Error::new(ErrorKind::Other, "An error"));
        };
    }

    fn add_response(&mut self, r: &[u8]) {
        let mut new_resp = Vec::with_capacity(r.len());

        for b in r.iter() {
            new_resp.push(*b);
        }
        self.response_queue.push_back(new_resp);
    }
}

#[cfg(test)]
mod test {
    use crate::{MockableSerial, MockableSerialBuilder, SerialMock};
    use std::collections::VecDeque;

    fn read_resp(p: &mut MockableSerial) -> Vec<u8> {
        let mut final_buffer = Vec::new();

        loop {
            let mut read_buf = [0; 1];

            match p.read(read_buf.as_mut_slice()) {
                Ok(_) => {
                    let byte = read_buf[0];

                    final_buffer.push(byte);

                    if byte == 0x23 as u8 {
                        break;
                    }
                }
                Err(e) => panic!("Unknown error occurred: {:?}", e),
            }
        }

        final_buffer
    }

    #[test]
    fn test_init() {
        let m = MockableSerialBuilder::new("/dev/null", 115200, 0x35, 1, None);
        let port = m.open_native();

        assert_eq!(port.address, "/dev/null");
        assert_eq!(port.baud, 115200);
        assert_eq!(port.stop_byte, 0x35);
        assert_eq!(port.read_n_bytes, 1);
    }

    #[test]
    fn test_add_response() {
        let m = MockableSerialBuilder::new("/dev/null", 115200, 0x35, 1, None);
        let mut port = m.open_native();
        port.add_response(&[0x65, 0x65, 0x65]);

        assert_eq!(port.response_queue.get(0).unwrap(), &vec![0x65, 0x65, 0x65]);
    }

    #[test]
    fn test_read() {
        let m = MockableSerialBuilder::new("/dev/null", 115200, 0x23, 1, None);
        let mut port = m.open_native();
        port.add_response(&[
            0x68, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64, 0x23,
        ]);
        let final_buffer = read_resp(&mut port);
        assert_eq!(std::str::from_utf8(&final_buffer).unwrap(), "hello world#");
    }

    #[test]
    fn test_initial_response() {
        let init_resp = VecDeque::from([vec![0x65, 0x65, 0x65], vec![0x64, 0x64, 0x64]]);
        let m = MockableSerialBuilder::new("/dev/null", 115200, 0x35, 1, Some(init_resp));
        let port = m.open_native();

        assert_eq!(port.response_queue.get(0).unwrap(), &vec![0x65, 0x65, 0x65]);
        assert_eq!(port.response_queue.get(1).unwrap(), &vec![0x64, 0x64, 0x64]);
    }

    #[test]
    fn test_multiple_responses() {
        let init_resp = VecDeque::from([
            vec![0x74, 0x65, 0x73, 0x74, 0x31, 0x23],
            vec![0x74, 0x65, 0x73, 0x74, 0x32, 0x23],
        ]);
        let m = MockableSerialBuilder::new("/dev/null", 115200, 0x23, 1, Some(init_resp));
        let mut port = m.open_native();

        let resp1 = read_resp(&mut port);
        let resp2 = read_resp(&mut port);

        assert_eq!(std::str::from_utf8(&resp1).unwrap(), "test1#");
        assert_eq!(std::str::from_utf8(&resp2).unwrap(), "test2#");
    }
}

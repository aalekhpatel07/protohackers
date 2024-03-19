use std::io::{BufReader, Read, BufRead};
use crate::error::de::Error;


#[derive(Debug)]

pub struct Protohackers<R: Read> {
    reader: BufReader<R>,
}

impl<R: Read> Protohackers<R> {

    pub fn from_reader(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
        }
    }

    pub fn peek_u8(&mut self) -> Result<u8, Error> {
        let buffer = self.reader.fill_buf()?;
        if buffer.is_empty() {
            return Err(Error::UnexpectedEOF);
        }
        Ok(buffer[0])
    }

    pub fn parse_u8(&mut self) -> Result<u8, Error> {
        let buffer = self.reader.fill_buf()?;
        if buffer.is_empty() {
            return Err(Error::UnexpectedEOF);
        }
        let buffer = [buffer[0]];
        self.reader.consume(1);
        Ok(buffer[0])
    }
    pub fn parse_u16(&mut self) -> Result<u16, Error> {
        let buffer = self.reader.fill_buf()?;
        if buffer.len() < 2 {
            return Err(Error::UnexpectedEOF);
        }
        let buffer = [buffer[0], buffer[1]];
        self.reader.consume(2);
        Ok(u16::from_be_bytes(buffer))
    }

    pub fn parse_u32(&mut self) -> Result<u32, Error> {
        let buffer = self.reader.fill_buf()?;
        if buffer.len() < 4 {
            return Err(Error::UnexpectedEOF);
        }
        let buffer = [buffer[0], buffer[1], buffer[2], buffer[3]];
        self.reader.consume(4);
        Ok(u32::from_be_bytes(buffer))
    }

    pub fn parse_u64(&mut self) -> Result<u64, Error> {
        let buffer = self.reader.fill_buf()?;
        if buffer.len() < 8 {
            return Err(Error::UnexpectedEOF);
        }
        let buffer = [buffer[0], buffer[1], buffer[2], buffer[3], buffer[4], buffer[5], buffer[6], buffer[7]];
        self.reader.consume(8);
        Ok(u64::from_be_bytes(buffer))
    }

    pub fn parse_char(&mut self) -> Result<char, Error> {
        Ok(self.parse_u8()? as char)
    }

    pub fn parse_str(&mut self) -> Result<String, Error> {
        let length = self.parse_u8()?;
        (0..length)
        .map(|_| self.parse_char())
        .collect()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let contents = b"\x05hello\x00\x00\x00\x40";
        let cursor = std::io::Cursor::new(contents);
        let mut ph = Protohackers::<>::from_reader(cursor);
        assert_eq!(ph.parse_str().unwrap(), "hello".to_string());
        assert_eq!(ph.parse_u32().unwrap(), 0x40);
    }
}
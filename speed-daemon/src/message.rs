use arrayvec::ArrayString;
use core::fmt;
use eyre::eyre;
use std::str::{from_utf8_unchecked, FromStr};
use tokio_util::{
    bytes::{Buf, BufMut},
    codec::{Decoder, Encoder},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MessageCodec;

type String = arrayvec::ArrayString<255>;

impl Encoder<Message> for MessageCodec {
    type Error = eyre::Error;

    fn encode(
        &mut self,
        item: Message,
        dst: &mut tokio_util::bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        match item {
            Message::Error(data) => {
                dst.put_u8(0x10);
                dst.put_str(&data.message)?;
            }
            Message::Plate(data) => {
                dst.put_u8(0x20);
                dst.put_str(&data.plate)?;
                dst.put_u32(data.timestamp);
            }
            Message::Ticket(data) => {
                dst.put_u8(0x21);
                dst.put_str(&data.plate)?;
                dst.reserve(16);
                dst.put_u16(data.road);
                dst.put_u16(data.mile1);
                dst.put_u32(data.timestamp1);
                dst.put_u16(data.mile2);
                dst.put_u32(data.timestamp2);
                dst.put_u16(data.speed);
            }
            Message::WantHeartbeat(data) => {
                dst.put_u8(0x40);
                dst.put_u32(data.interval);
            }
            Message::Heartbeat(_) => {
                dst.put_u8(0x41);
            }
            Message::IAmCamera(data) => {
                dst.reserve(7);
                dst.put_u8(0x80);
                dst.put_u16(data.road);
                dst.put_u16(data.mile);
                dst.put_u16(data.limit);
            }
            Message::IAmDispatcher(data) => {
                dst.reserve(2 + data.roads.len() * 2);
                dst.put_u8(0x81);
                dst.put_u8(data.num_roads);
                dst.extend(data.roads.iter().flat_map(|&road| road.to_be_bytes()))
            }
        }
        Ok(())
    }
}

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = eyre::Error;

    fn decode(
        &mut self,
        src: &mut tokio_util::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            // no header byte to parse a message.
            return Ok(None);
        }

        let msg_type = src[0];
        // We'll consume the header in our current attempt
        // in case we choose to commit the read.
        let mut consumed = 1;

        match msg_type {
            // Error
            0x10 => {
                let Some((error_str, bytes_read)) = (&src[consumed..]).read_str()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                // commit the read.
                src.advance(consumed);

                let message = Message::Error(Error { message: error_str });
                Ok(Some(message))
            }
            // Plate
            0x20 => {
                let Some((plate, bytes_read)) = (&src[consumed..]).read_str()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                let Some((timestamp, bytes_read)) = (&src[consumed..]).read_u32()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                // commit the read.
                src.advance(consumed);

                let message = Message::Plate(Plate { plate, timestamp });
                Ok(Some(message))
            }
            // Ticket
            0x21 => {
                let Some((plate, bytes_read)) = (&src[consumed..]).read_str()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                let Some((road, bytes_read)) = (&src[consumed..]).read_u16()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                let Some((mile1, bytes_read)) = (&src[consumed..]).read_u16()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                let Some((timestamp1, bytes_read)) = (&src[consumed..]).read_u32()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                let Some((mile2, bytes_read)) = (&src[consumed..]).read_u16()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                let Some((timestamp2, bytes_read)) = (&src[consumed..]).read_u32()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                let Some((speed, bytes_read)) = (&src[consumed..]).read_u16()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                // commit the read.
                src.advance(consumed);

                let message = Message::Ticket(Ticket {
                    plate,
                    road,
                    mile1,
                    timestamp2,
                    speed,
                    timestamp1,
                    mile2,
                });
                Ok(Some(message))
            }

            // WantHeartbeat
            0x40 => {
                let Some((interval, bytes_read)) = (&src[consumed..]).read_u32()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                // commit the read.
                src.advance(consumed);

                let message = Message::WantHeartbeat(WantHeartbeat { interval });
                Ok(Some(message))
            }
            // Heartbeat
            0x41 => {
                // commit the read.
                src.advance(consumed);

                let message = Message::Heartbeat(Heartbeat);
                Ok(Some(message))
            }
            // IAmCamera
            0x80 => {
                let Some((road, bytes_read)) = (&src[consumed..]).read_u16()? else {
                    return Ok(None);
                };
                consumed += bytes_read;
                let Some((mile, bytes_read)) = (&src[consumed..]).read_u16()? else {
                    return Ok(None);
                };
                consumed += bytes_read;
                let Some((limit, bytes_read)) = (&src[consumed..]).read_u16()? else {
                    return Ok(None);
                };
                consumed += bytes_read;

                // commit the read.
                src.advance(consumed);

                let message = Message::IAmCamera(IAmCamera { road, mile, limit });
                Ok(Some(message))
            }
            // IAmDispatcher
            0x81 => {
                let Some((num_roads, bytes_read)) = (&src[consumed..]).read_u8()? else {
                    return Ok(None);
                };
                consumed += bytes_read;
                let mut roads: arrayvec::ArrayVec<u16, 255> = arrayvec::ArrayVec::new();

                for _ in 0..(num_roads as usize) {
                    let Some((road, bytes_read)) = (&src[consumed..]).read_u16()? else {
                        return Ok(None);
                    };
                    roads.push(road);
                    consumed += bytes_read;
                }

                // commit the read.
                src.advance(consumed);

                let message = Message::IAmDispatcher(IAmDispatcher { num_roads, roads });
                Ok(Some(message))
            }
            unknown => Err(eyre!("Unrecognized message type header: {unknown:#04X}")),
        }
    }
}

pub trait ReadFull {
    type Error;
    fn read_str(&self) -> Result<Option<(ArrayString<255>, usize)>, Self::Error>;
    fn read_u8(&self) -> Result<Option<(u8, usize)>, Self::Error>;
    fn read_u16(&self) -> Result<Option<(u16, usize)>, Self::Error>;
    fn read_u32(&self) -> Result<Option<(u32, usize)>, Self::Error>;
}

impl<B> ReadFull for B
where
    B: tokio_util::bytes::Buf + Clone,
{
    type Error = eyre::Error;

    fn read_str(&self) -> Result<Option<(ArrayString<255>, usize)>, Self::Error> {
        // Currently Buf doesn't provide a way to
        // peek at a few bytes (i.e. without advancing internal cursor)
        // so we clone to prevent advancing our cursor until we really need to.
        // https://github.com/tokio-rs/bytes/issues/382
        let mut temp = self.clone();

        if !temp.has_remaining() {
            // Nothing to read.
            return Ok(None);
        }
        let strlen = temp.get_u8();

        let mut s = Vec::with_capacity(strlen as usize);
        for _ in 0..(strlen as usize) {
            if !temp.has_remaining() {
                // Not enough bytes to form complete string.
                return Ok(None);
            }
            s.push(temp.get_u8());
        }

        // Now we have enough bytes to read the complete string.
        let s = unsafe { from_utf8_unchecked(&s) };
        let data = ArrayString::from_str(s)?;
        Ok(Some((data, data.len() + 1)))
    }

    fn read_u8(&self) -> Result<Option<(u8, usize)>, Self::Error> {
        let mut temp = self.clone();
        if !temp.has_remaining() {
            // Nothing to read.
            return Ok(None);
        }
        Ok(Some((temp.get_u8(), 1)))
    }

    fn read_u16(&self) -> Result<Option<(u16, usize)>, Self::Error> {
        let mut temp = self.clone();
        let mut raw = [0u8; 2];

        #[allow(clippy::needless_range_loop)]
        for idx in 0..2 {
            if !temp.has_remaining() {
                // Not enough bytes to read full u16.
                return Ok(None);
            }
            raw[idx] = temp.get_u8();
        }

        Ok(Some((u16::from_be_bytes(raw), 2)))
    }

    fn read_u32(&self) -> Result<Option<(u32, usize)>, Self::Error> {
        let mut temp = self.clone();
        let mut raw = [0u8; 4];

        #[allow(clippy::needless_range_loop)]
        for idx in 0..4 {
            if !temp.has_remaining() {
                // Not enough bytes to read full u16.
                return Ok(None);
            }
            raw[idx] = temp.get_u8();
        }
        Ok(Some((u32::from_be_bytes(raw), 4)))
    }
}

pub trait PutStr {
    fn put_str(&mut self, s: &arrayvec::ArrayString<255>) -> fmt::Result;
}

impl<B> PutStr for B
where
    B: tokio_util::bytes::BufMut,
{
    fn put_str(&mut self, s: &arrayvec::ArrayString<255>) -> fmt::Result {
        if self.remaining_mut() >= (s.len() + 1) {
            self.put_u8(s.len() as u8);
            self.put_slice(s.as_bytes());
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plate {
    pub plate: String,
    pub timestamp: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub plate: String,
    pub road: u16,
    pub mile1: u16,
    pub timestamp1: u32,
    pub mile2: u16,
    pub timestamp2: u32,
    pub speed: u16,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct WantHeartbeat {
    pub interval: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Heartbeat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IAmCamera {
    pub road: u16,
    pub mile: u16,
    pub limit: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IAmDispatcher {
    pub num_roads: u8,
    pub roads: arrayvec::ArrayVec<u16, 255>,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Message {
    Error(Error),
    Plate(Plate),
    Ticket(Ticket),
    WantHeartbeat(WantHeartbeat),
    Heartbeat(Heartbeat),
    IAmCamera(IAmCamera),
    IAmDispatcher(IAmDispatcher),
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrayvec::ArrayVec;
    use futures_util::{SinkExt, StreamExt};
    use test_case::test_case;
    use tokio_util::codec::{FramedRead, FramedWrite};

    #[test_case(
        Message::Error(Error { message: "aab".try_into().unwrap() }), 
        vec![0x10, 0x03, 0x61, 0x61, 0x62]; 
        "Error"
    )]
    #[test_case(
        Message::Plate(Plate { plate: "aab".try_into().unwrap(), timestamp: 100 }), 
        vec![0x20, 0x03, 0x61, 0x61, 0x62, 0x00, 0x00, 0x00, 0x64]; 
        "Plate"
    )]
    #[test_case(
        Message::Ticket(Ticket { plate: "aab".try_into().unwrap(), road: 10, mile1: 20, timestamp1: 30, mile2: 40, timestamp2: 50, speed: 60 }), 
        vec![
            0x21, 
            0x03, 0x61, 0x61, 0x62, 
            0x00, 0x0A,
            0x00, 0x14,
            0x00, 0x00, 0x00, 0x1E,
            0x00, 0x28,
            0x00, 0x00, 0x00, 0x32,
            0x00, 0x3C,
        ]; 
        "Ticket"
    )]
    #[test_case(
        Message::WantHeartbeat(WantHeartbeat { interval: 256 }), 
        vec![0x40, 0x00, 0x00, 0x01, 0x00]; 
        "WantHeartbeat"
    )]
    #[test_case(
        Message::Heartbeat(Heartbeat), 
        vec![0x41]; 
        "Heartbeat"
    )]
    #[test_case(
        Message::IAmCamera(IAmCamera { road: 10, mile: 20, limit: 30 }), 
        vec![
            0x80, 
            0x00, 0x0A, 
            0x00, 0x14,
            0x00, 0x1E
        ]; 
        "IAmCamera"
    )]
    #[test_case(
        Message::IAmDispatcher(IAmDispatcher { num_roads: 3, roads: ArrayVec::from_iter([10, 20, 30]) }), 
        vec![
            0x81,
            0x03, 
            0x00, 0x0A, 
            0x00, 0x14,
            0x00, 0x1E
        ]; 
        "IAmDispatcher"
    )]
    #[tokio::test]
    async fn test_encode(message: Message, expected: Vec<u8>) {
        let mut framed_write = FramedWrite::new(Vec::new(), MessageCodec);
        let res = framed_write.send(message).await;
        assert!(res.is_ok());
        res.unwrap();
        let observed = framed_write.into_inner();
        assert_eq!(observed, expected);
    }

    #[tokio::test]
    async fn test_encode_stream() {
        let messages = vec![
            Message::Plate(Plate {
                plate: String::from("abc").unwrap(),
                timestamp: 10,
            }),
            Message::WantHeartbeat(WantHeartbeat { interval: 12 }),
        ];
        let mut framed_write = FramedWrite::new(Vec::new(), MessageCodec);
        for frame in messages {
            framed_write.feed(frame).await.expect("encode message")
        }
        framed_write.flush().await.expect("flush messages");

        let dest = framed_write.into_inner();
        assert_eq!(
            &dest,
            &[
                // Message beings
                // ---- Message is a Plate!
                0x20, // ------ plate has 3 chars: "abc"
                0x03, 0x61, 0x62, 0x63, // ------ timestamp = 10 (4 bytes)
                0x00, 0x00, 0x00, 0x0A,
                // End of Message: Plate { plate: "abc", timestamp: 10 }

                // Message beings
                // ---- Message is a WantHeartbeat!
                0x40, // ------ interval = 12 (4 bytes)
                0x00, 0x00, 0x00, 0x0C,
                // End of Message: WantHeartbeat { interval: 12 }
            ]
        );
    }

    #[test_case(
        vec![0x10, 0x03, 0x61, 0x61, 0x62],
        Message::Error(Error { message: "aab".try_into().unwrap() });
        "Error"
    )]
    #[test_case(
        vec![0x20, 0x03, 0x61, 0x61, 0x62, 0x00, 0x00, 0x00, 0x64],
        Message::Plate(Plate { plate: "aab".try_into().unwrap(), timestamp: 100 });
        "Plate"
    )]
    #[test_case(
        vec![
            0x21, 
            0x03, 0x61, 0x61, 0x62, 
            0x00, 0x0A,
            0x00, 0x14,
            0x00, 0x00, 0x00, 0x1E,
            0x00, 0x28,
            0x00, 0x00, 0x00, 0x32,
            0x00, 0x3C,
        ],
        Message::Ticket(Ticket { plate: "aab".try_into().unwrap(), road: 10, mile1: 20, timestamp1: 30, mile2: 40, timestamp2: 50, speed: 60 });
        "Ticket"
    )]
    #[test_case(
        vec![0x40, 0x00, 0x00, 0x01, 0x00],
        Message::WantHeartbeat(WantHeartbeat { interval: 256 });
        "WantHeartbeat"
    )]
    #[test_case(
        vec![0x41],
        Message::Heartbeat(Heartbeat);
        "Heartbeat"
    )]
    #[test_case(
        vec![
            0x80, 
            0x00, 0x0A, 
            0x00, 0x14,
            0x00, 0x1E
        ],
        Message::IAmCamera(IAmCamera { road: 10, mile: 20, limit: 30 });
        "IAmCamera"
    )]
    #[test_case(
        vec![
            0x81,
            0x03, 
            0x00, 0x0A, 
            0x00, 0x14,
            0x00, 0x1E
        ],
        Message::IAmDispatcher(IAmDispatcher { num_roads: 3, roads: ArrayVec::from_iter([10, 20, 30]) });
        "IAmDispatcher"
    )]
    #[tokio::test]
    async fn test_decode(contents: Vec<u8>, expected: Message) {
        let cursor = std::io::Cursor::new(contents);
        let mut framed_read = FramedRead::new(cursor, MessageCodec);
        let observed = framed_read.next().await.unwrap().expect("decode message");
        assert_eq!(observed, expected);
        assert!(framed_read.next().await.is_none());
    }

    #[test]
    fn test_put_str() {
        let mut data = tokio_util::bytes::BytesMut::new();
        data.put_str(&("abc".try_into().unwrap()))
            .expect("put_str works");
        assert_eq!(data.to_vec(), vec![0x03, 0x61, 0x62, 0x63]);
    }

    #[test]
    fn test_put_str_error() {
        // let buffer = [0u8; 5];
        // let mut contents = tokio_util::bytes::BytesMut::from_iter(buffer);
        let mut dst = [0; 4];
        let mut buf = &mut dst[..];
        // The buffer is only 4 bytes wide
        assert_eq!(buf.remaining_mut(), 4);
        // but we want to write 5 chars, so it should err!
        let res = buf.put_str(&("abcde".try_into().unwrap()));
        assert_eq!(res, Err(core::fmt::Error));
    }

    #[test_case(
        vec![
            0x10, 
            0x03, 0x61, // 0x61, 0x62
        ],
        Message::Error(Error { message: "aab".try_into().unwrap() });
        "Error"
    )]
    #[test_case(
        vec![
            0x20, 
            0x03, 0x61, 0x61, 0x62, 
            // 0x00, 0x00, 0x00, 0x64
        ],
        Message::Plate(Plate { plate: "aab".try_into().unwrap(), timestamp: 100 });
        "Plate"
    )]
    #[test_case(
        vec![
            0x21, 
            0x03, 0x61, 0x61, 0x62, 
            0x00, 0x0A,
            0x00, 0x14,
            0x00, 0x00, 0x00, 0x1E,
            0x00, 0x28,
            0x00, 0x00, 0x00, 0x32,
            // 0x00, 0x3C,
        ],
        Message::Ticket(Ticket { plate: "aab".try_into().unwrap(), road: 10, mile1: 20, timestamp1: 30, mile2: 40, timestamp2: 50, speed: 60 });
        "Ticket"
    )]
    #[test_case(
        vec![
            0x40, 
            0x00, 0x00, // 0x01, 0x00
        ],
        Message::WantHeartbeat(WantHeartbeat { interval: 256 });
        "WantHeartbeat"
    )]
    #[test_case(
        vec![
            0x80, 
            0x00, 0x0A, 
            0x00, 0x14,
            // 0x00, 0x1E
        ],
        Message::IAmCamera(IAmCamera { road: 10, mile: 20, limit: 30 });
        "IAmCamera"
    )]
    #[test_case(
        vec![
            0x81,
            0x03, 
            0x00, 0x0A, 
            0x00, 0x14,
            // 0x00, 0x1E
        ],
        Message::IAmDispatcher(IAmDispatcher { num_roads: 3, roads: ArrayVec::from_iter([10, 20, 30]) });
        "IAmDispatcher"
    )]
    #[tokio::test]
    async fn test_decode_incomplete(partial_contents: Vec<u8>, expected: Message) {
        let cursor = std::io::Cursor::new(partial_contents);
        let mut framed_read = FramedRead::new(cursor, MessageCodec);
        // let variant_name = std::any::type_name_of_val(&expected).to_string();
        let variant_name = format!("{:?}", expected);
        _ = framed_read.next().await.unwrap().expect_err(
            format!("should fail to decode partial contents as {}", variant_name).as_str(),
        );
    }
}

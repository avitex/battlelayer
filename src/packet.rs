use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::convert::TryFrom;
use std::io::Cursor;
use std::str;

const PACKET_MAX_SIZE: usize = 16384;
const PACKET_MAX_WORDS: usize = 256;
const PACKET_HEADER_SIZE: usize = 12;
const PACKET_WORD_MIN_SIZE: usize = 5;
const PACKET_WORD_MAX_SIZE: usize = PACKET_MAX_SIZE - (PACKET_HEADER_SIZE + PACKET_WORD_MIN_SIZE);
const PACKET_SEQ_CLIENT_MASK_U32: u32 = 0x8000_0000;
const PACKET_SEQ_RESPON_MASK_U32: u32 = 0x4000_0000;
const PACKET_SEQ_HEADER_MASK_U32: u32 = PACKET_SEQ_CLIENT_MASK_U32 | PACKET_SEQ_RESPON_MASK_U32;

/// Reads a packet's wire representation from a BytesMut.
pub fn read_packet(buf: &mut BytesMut) -> Result<Option<Packet>, PacketError> {
    // Return early if we cannot fullfill the packet header size.
    if buf.len() < PACKET_HEADER_SIZE {
        return Ok(None);
    }
    // Split off the packet header to a seperate buf.
    let header_buf = buf.split_to(PACKET_HEADER_SIZE);
    // Create a cursor to read the header.
    let mut header_cur = Cursor::new(header_buf.as_ref());
    // Read the packet sequence.
    let seq = PacketSequence::from_raw(header_cur.get_u32_le());
    // Read the packet size.
    let size = read_u32_as_bounded_usize(&mut header_cur, PACKET_HEADER_SIZE, PACKET_MAX_SIZE)?;
    // Read the word count.
    let word_count = read_u32_as_bounded_usize(&mut header_cur, 0, PACKET_MAX_WORDS)?;
    // Create a container for the packet words.
    let mut words = Vec::with_capacity(word_count);
    // Calculate the body size.
    let body_size = size - PACKET_HEADER_SIZE;
    // Return early if we can't met the packet body size.
    // We also rejoin the header buf, for another attempt.
    if buf.len() < body_size {
        buf.unsplit(header_buf);
        return Ok(None);
    }
    // Read the body bytes.
    let mut body_buf = buf.split_to(body_size);
    // Read packet words.
    for _ in 0..word_count {
        // Validate we can read the size of the word.
        // If a previous word claimed to be larger than it was
        // in reality, this will ensure we don't panic.
        if body_buf.len() < 4 {
            return Err(PacketError::Malformed);
        }
        // Split off the word size from the body buf.
        let word_size_buf = body_buf.split_to(4);
        // Extract the word size from the word size buf.
        let word_size = read_u32_as_bounded_usize(
            &mut Cursor::new(word_size_buf.as_ref()),
            PACKET_WORD_MIN_SIZE,
            PACKET_WORD_MAX_SIZE,
        )?;
        // Again validate we can read the claimed size
        // of the word, including the NULL terminator.
        if body_buf.len() < word_size + 1 {
            return Err(PacketError::Malformed);
        }
        // Split off the word content from the body buf.
        let word_buf = body_buf.split_to(word_size);
        // Split off and validate we have a trailing null character.
        if body_buf.split_to(1).as_ref() != &[0] {
            return Err(PacketError::Malformed);
        }
        // Freeze the word bytes.
        let word_bytes = word_buf.freeze();
        // Push the packet word to the container.
        words.push(PacketWord::from_bytes(word_bytes)?);
    }
    Ok(Some(Packet { seq, words }))
}

/// Writes a packet's wire representation into a BytesMut.
pub fn write_packet(buf: &mut BytesMut, packet: Packet) -> Result<(), PacketError> {
    // Get the total calculated packet size.
    let packet_size = packet.byte_size();
    // Reserve the required space within the buf.
    buf.reserve(packet_size);
    // Write the packet sequence to the buf.
    buf.put_u32_le(packet.seq.to_raw());
    // Write the packet size to the buf.
    write_size_u32(buf, packet_size)?;
    // Write the packet word count to the buf.
    write_size_u32(buf, packet.words.len())?;
    // For each word:
    for word in packet.words.into_iter() {
        // Write the word size to the buf.
        write_size_u32(buf, word.byte_size())?;
        // Write the word content to the buf.
        buf.put(word.into_bytes());
        // Write the NULL term to the buf.
        buf.put_u8(0);
    }
    Ok(())
}

///////////////////////////////////////////////////////////////////////////////

fn write_size_u32(buf: &mut BytesMut, size: usize) -> Result<(), PacketError> {
    // Validate the usize will fit inside a u32.
    if size > (u32::max_value() as usize) {
        return Err(PacketError::InvalidSize(size));
    }
    // Write the size to the buf.
    buf.put_u32_le(size as u32);
    Ok(())
}

fn read_u32_as_bounded_usize(
    mut buf: impl Buf,
    min: usize,
    max: usize,
) -> Result<usize, PacketError> {
    // Extract the value.
    let val = buf.get_u32_le() as usize;
    // Validate it is within the bounds.
    if val < min || val > max {
        return Err(PacketError::InvalidSize(val));
    }
    Ok(val)
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Packet {
    pub seq: PacketSequence,
    pub words: Vec<PacketWord>,
}

impl Packet {
    /// Calculates the total size of the packet.
    pub fn byte_size(&self) -> usize {
        // Calculate the wire representation size of
        // the words.
        let words_byte_size: usize = self
            .words
            .iter()
            .map(|w| w.byte_size() + PACKET_WORD_MIN_SIZE)
            .sum();
        // Return the sum of the words and packet header.
        words_byte_size + PACKET_HEADER_SIZE
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Represents a failure while handling a packet.
#[derive(Debug)]
pub enum PacketError {
    Malformed,
    InvalidSize(usize),
    InvalidWordChar(u8),
    InvalidSequenceNumber,
}

#[derive(Debug, PartialEq)]
pub enum PacketKind {
    /// Indicates the packet forms a request.
    Request,
    /// Indicates the packet forms a response.
    Response,
}

#[derive(Debug, PartialEq)]
pub enum PacketOrigin {
    /// Indicates the packet originated from the server.
    Server,
    /// Indicates the packet originated from the client.
    Client,
}

////////////////////////////////////////////////////////////////////////////////

/// The sequence structure of a packet.
#[derive(Debug)]
pub struct PacketSequence {
    raw: u32,
}

impl PacketSequence {
    /// Creates a new packet sequence.
    pub fn new(kind: PacketKind, origin: PacketOrigin, mut seq: u32) -> Result<Self, PacketError> {
        if (seq & PACKET_SEQ_HEADER_MASK_U32) != 0 {
            Err(PacketError::InvalidSequenceNumber)
        } else {
            if kind == PacketKind::Response {
                seq |= PACKET_SEQ_RESPON_MASK_U32;
            }
            if origin == PacketOrigin::Client {
                seq |= PACKET_SEQ_CLIENT_MASK_U32;
            }
            Ok(Self::from_raw(seq))
        }
    }

    /// Create a new packet sequence from it's raw protocol
    /// representation.
    pub fn from_raw(raw: u32) -> Self {
        Self { raw }
    }

    /// Returns the raw protocol representation.
    pub fn to_raw(&self) -> u32 {
        self.raw
    }

    /// Returns the origin of the packet (client/server).
    pub fn origin(&self) -> PacketOrigin {
        if (self.raw & PACKET_SEQ_CLIENT_MASK_U32) != 0 {
            PacketOrigin::Client
        } else {
            PacketOrigin::Server
        }
    }

    /// Returns the kind of packet (request/response).
    pub fn kind(&self) -> PacketKind {
        if (self.raw & PACKET_SEQ_RESPON_MASK_U32) != 0 {
            PacketKind::Response
        } else {
            PacketKind::Request
        }
    }

    /// Returns the packet sequence number.
    pub fn number(&self) -> u32 {
        self.raw & !PACKET_SEQ_HEADER_MASK_U32
    }
}

////////////////////////////////////////////////////////////////////////////////

/// The word structure of a packet.
#[derive(Debug, PartialEq, Clone)]
pub struct PacketWord {
    bytes: Bytes,
}

impl PacketWord {
    /// Create a packet word from a UTF8 string.
    pub fn new(word: &str) -> Result<Self, PacketError> {
        Self::from_bytes(Bytes::from(word.as_bytes()))
    }

    /// Create a packet word from bytes.
    pub fn from_bytes(bytes: Bytes) -> Result<Self, PacketError> {
        if let Some(invalid_char) = bytes.as_ref().iter().find(|b| !Self::is_valid_char(**b)) {
            Err(PacketError::InvalidWordChar(*invalid_char))
        } else {
            Ok(Self { bytes })
        }
    }

    /// Checks if packet word char is in ASCII range and is not NULL.
    pub fn is_valid_char(byte: u8) -> bool {
        byte != 0u8 && byte.is_ascii()
    }

    /// Get the UTF8 representation of the packet word.
    pub fn as_str(&self) -> &str {
        // Safe as we validate each character on contruction.
        unsafe { str::from_utf8_unchecked(self.as_ref()) }
    }

    /// Consume the packet word as bytes.
    pub fn into_bytes(self) -> Bytes {
        self.bytes
    }

    /// Total size of the word content.
    pub fn byte_size(&self) -> usize {
        self.bytes.len()
    }
}

impl AsRef<[u8]> for PacketWord {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_ref()
    }
}

impl TryFrom<&str> for PacketWord {
    type Error = PacketError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl TryFrom<String> for PacketWord {
    type Error = PacketError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_bytes(Bytes::from(s))
    }
}

impl TryFrom<&[u8]> for PacketWord {
    type Error = PacketError;

    fn try_from(s: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(Bytes::from(s))
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[rustfmt::skip]
    fn basic_packet_read_write_test() {
        let packet_bytes = &[
            // seq
            0b0000_0000, 0b0000_0000,
            0b0000_0000, 0b1000_0000,
            // size,
            32, 0, 0, 0,
            // word num
            2, 0, 0, 0,
            // word "hello"
            5, 0, 0, 0, b'h', b'e', b'l', b'l', b'o', 0,
            // word "world"
            5, 0, 0, 0, b'w', b'o', b'r', b'l', b'd', 0,
        ];
        let packet = read_packet(&mut BytesMut::from(&packet_bytes[..])).unwrap().unwrap();
        assert_eq!(packet.seq.kind(), PacketKind::Request);
        assert_eq!(packet.seq.origin(), PacketOrigin::Client);
        assert_eq!(
            &packet.words[..],
            &[
                PacketWord::new("hello").unwrap(),
                PacketWord::new("world").unwrap(),
            ]
        );
        let mut out = BytesMut::with_capacity(packet_bytes.len());
        write_packet(&mut out, packet).unwrap();
        assert_eq!(&out[..], &packet_bytes[..]);
    }

    #[test]
    fn packet_sequence_number_test() {
        let seq = PacketSequence::new(PacketKind::Request, PacketOrigin::Client, 1234u32).unwrap();
        assert_eq!(seq.number(), 1234u32);
    }

    #[test]
    #[should_panic]
    fn packet_sequence_number_invalid_test() {
        PacketSequence::new(PacketKind::Request, PacketOrigin::Client, 0xffffffff).unwrap();
    }

    #[test]
    fn client_packet_sequence_test() {
        let seq_bytes = u32::from_le_bytes([0b0000_00000, 0b0000_0000, 0b0000_0000, 0b1000_0000]);
        let seq = PacketSequence::from_raw(seq_bytes);
        assert_eq!(seq.origin(), PacketOrigin::Client);
        assert_eq!(seq.number(), 0u32);
        assert_eq!(seq.kind(), PacketKind::Request);
    }

    #[test]
    fn server_packet_sequence_test() {
        let seq_bytes = u32::from_le_bytes([0b0000_00000, 0b0000_0000, 0b0000_0000, 0b0100_0000]);
        let seq = PacketSequence::from_raw(seq_bytes);
        assert_eq!(seq.origin(), PacketOrigin::Server);
        assert_eq!(seq.kind(), PacketKind::Response);
        assert_eq!(seq.number(), 0u32);
    }
}

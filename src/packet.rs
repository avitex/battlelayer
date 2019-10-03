use std::{io, str};

const PACKET_MAX_SIZE: usize = 16384;
const PACKET_MAX_WORDS: usize = 256;
const PACKET_HEADER_SIZE: usize = 12;
const PACKET_WORD_MIN_SIZE: usize = 5;
const PACKET_WORD_MAX_SIZE: usize = PACKET_MAX_SIZE - (PACKET_HEADER_SIZE + PACKET_WORD_MIN_SIZE);
const PACKET_SEQ_CLIENT_MASK_U8: u8 = 0x80;
const PACKET_SEQ_RESPON_MASK_U8: u8 = 0x40;
const PACKET_SEQ_CLIENT_MASK_U32: u32 = 0x8000_0000;
const PACKET_SEQ_RESPON_MASK_U32: u32 = 0x4000_0000;
const PACKET_SEQ_HEADER_MASK_U32: u32 = PACKET_SEQ_CLIENT_MASK_U32 | PACKET_SEQ_RESPON_MASK_U32;

////////////////////////////////////////////////////////////////////////////////

fn read_exact(mut r: impl io::Read, buf: &mut [u8]) -> Result<(), PacketError> {
    Ok(r.read_exact(buf)?)
}

fn read_quad(r: impl io::Read) -> Result<[u8; 4], PacketError> {
    let mut buf = [0u8; 4];
    read_exact(r, &mut buf)?;
    Ok(buf)
}

fn read_u32_size(r: impl io::Read, max: usize) -> Result<usize, PacketError> {
    let buf = read_quad(r)?;
    let val = u32::from_le_bytes(buf) as usize;
    if val > max {
        return Err(PacketError::InvalidSize(val));
    }
    Ok(val)
}

fn read_packet<'s>(
    mut r: impl io::Read,
    scratch: &'s mut Vec<u8>,
) -> Result<Packet<'s>, PacketError> {
    // Read the packet sequence.
    let seq_buf = read_quad(&mut r)?;
    let seq = PacketSequence::from_raw(seq_buf);
    // Read the packet size.
    let size = read_u32_size(&mut r, PACKET_MAX_SIZE)?;
    // Validate it is the header size or larger.
    if size < PACKET_HEADER_SIZE {
        return Err(PacketError::InvalidSize(size));
    }
    // Read the number of words.
    let num_word = read_u32_size(&mut r, PACKET_MAX_WORDS)?;
    // Init container for words.
    let mut words = Vec::with_capacity(num_word);
    // Calculate the body size.
    let body_size = size - PACKET_HEADER_SIZE;
    // Read the packet body into the scratch space.
    unsafe {
        // Firstly we clear the scratch space.
        scratch.clear();
        // Now we reserve enough space to write to.
        scratch.reserve(body_size);
        // We set the size of the space to that of which we reserved.
        // This memory is uninitialized, however we will write to
        // it in the next step.
        scratch.set_len(body_size);
        // Read the body bytes into the scratch space.
        read_exact(r, scratch.as_mut_slice())?;
    }
    // Read words from the scratch space.
    let mut scratch_cursor = 0;
    for _ in 0..num_word {
        if (scratch.len() - scratch_cursor) < PACKET_WORD_MIN_SIZE {
            return Err(PacketError::Malformed);
        }
        let word_size_end = scratch_cursor + 4;
        let word_size = read_u32_size(
            &scratch[scratch_cursor..word_size_end],
            PACKET_WORD_MAX_SIZE,
        )?;
        if (scratch.len() - scratch_cursor) < word_size {
            return Err(PacketError::Malformed);
        }
        let word_end = word_size_end + word_size;
        let word_bytes = &scratch[word_size_end..word_end];
        // Check the trailing null character.
        if scratch[word_end] != 0 {
            return Err(PacketError::Malformed);
        }
        scratch_cursor = word_end + 1;
        words.push(PacketWord::from_raw(word_bytes)?);
    }
    Ok(Packet { seq, words })
}

fn write_size_u32(mut w: impl io::Write, size: usize) -> Result<(), PacketError> {
    if size > (u32::max_value() as usize) {
        return Err(PacketError::InvalidSize(size));
    }
    let size_bytes = (size as u32).to_le_bytes();
    w.write(&size_bytes[..])?;
    Ok(())
}

fn write_packet<'a>(mut w: impl io::Write, p: &Packet<'a>) -> Result<(), PacketError> {
    w.write(p.seq.as_bytes())?;
    write_size_u32(&mut w, p.byte_size())?;
    write_size_u32(&mut w, p.words.len())?;
    for word in p.words.iter() {
        write_size_u32(&mut w, word.byte_size())?;
        w.write(word.as_bytes())?;
        w.write(&[0])?;
    }
    Ok(())
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Packet<'a> {
    pub seq: PacketSequence,
    pub words: Vec<PacketWord<'a>>,
}

impl<'a> Packet<'a> {
    /// Calculates the total size of the packet.
    pub fn byte_size(&self) -> usize {
        let words_byte_size: usize = self
            .words
            .iter()
            .map(|w| w.byte_size() + PACKET_WORD_MIN_SIZE)
            .sum();
        words_byte_size + PACKET_HEADER_SIZE
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Represents a failure while handling a packet.
#[derive(Debug)]
pub enum PacketError {
    Io(io::Error),
    BrokenPipe,
    Malformed,
    InvalidSize(usize),
    InvalidWordChar(u8),
    InvalidSequenceNumber,
}

impl From<io::Error> for PacketError {
    fn from(err: io::Error) -> Self {
        PacketError::Io(err)
    }
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

#[derive(Debug)]
pub struct PacketSequence {
    bytes: [u8; 4],
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
            Ok(Self::from_raw(seq.to_le_bytes()))
        }
    }

    /// Create a new packet sequence from it's raw protocol
    /// representation.
    pub fn from_raw(bytes: [u8; 4]) -> Self {
        Self { bytes }
    }

    /// Returns the origin of the packet (client/server).
    pub fn origin(&self) -> PacketOrigin {
        if (self.bytes[3] & PACKET_SEQ_CLIENT_MASK_U8) != 0 {
            PacketOrigin::Client
        } else {
            PacketOrigin::Server
        }
    }

    /// Returns the kind of packet (request/response).
    pub fn kind(&self) -> PacketKind {
        if (self.bytes[3] & PACKET_SEQ_RESPON_MASK_U8) != 0 {
            PacketKind::Response
        } else {
            PacketKind::Request
        }
    }

    /// Returns the packet sequence number.
    pub fn number(&self) -> u32 {
        u32::from_le_bytes(self.bytes) & !PACKET_SEQ_HEADER_MASK_U32
    }

    /// Returns the raw protocol representation.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..]
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Represents a Packet word.
#[derive(Debug, PartialEq)]
pub struct PacketWord<'a> {
    bytes: &'a [u8],
}

impl<'a> PacketWord<'a> {
    pub fn new(word: &'a str) -> Result<Self, PacketError> {
        Self::from_raw(word.as_bytes())
    }

    pub fn from_raw(bytes: &'a [u8]) -> Result<Self, PacketError> {
        if let Some(invalid_char) = bytes.into_iter().find(|b| !Self::is_valid_char(**b)) {
            Err(PacketError::InvalidWordChar(*invalid_char))
        } else {
            Ok(Self { bytes })
        }
    }

    /// Checks if is in ASCII range and is not NULL.
    pub fn is_valid_char(byte: u8) -> bool {
        byte != 0u8 && byte.is_ascii()
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.bytes) }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.bytes
    }

    /// Total size of the word content.
    pub fn byte_size(&self) -> usize {
        self.bytes.len()
    }
}

////////////////////////////////////////////////////////////////////////////////

pub struct PacketReader<R: io::Read> {
    r: R,
    broken: bool,
    scratch: Vec<u8>,
}

impl<R: io::Read> PacketReader<R> {
    pub fn new(r: R) -> Self {
        Self {
            r,
            broken: false,
            scratch: Vec::with_capacity(4096),
        }
    }

    pub fn read_packet<'p>(&'p mut self) -> Result<Packet<'p>, PacketError> {
        if self.broken {
            return Err(PacketError::BrokenPipe);
        }
        match read_packet(&mut self.r, &mut self.scratch) {
            Ok(p) => Ok(p),
            Err(err) => {
                self.broken = true;
                Err(err)
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

pub struct PacketWriter<W: io::Write> {
    w: W,
}

impl<W: io::Write> PacketWriter<W> {
    pub fn new(w: W) -> Self {
        Self { w }
    }

    pub fn write_packet<'a>(&mut self, p: &Packet<'a>) -> Result<(), PacketError> {
        write_packet(&mut self.w, p)
    }
}

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
        let mut scratch = Vec::new();
        let packet = read_packet(&packet_bytes[..], &mut scratch).unwrap();
        assert_eq!(packet.seq.kind(), PacketKind::Request);
        assert_eq!(packet.seq.origin(), PacketOrigin::Client);
        assert_eq!(
            &packet.words[..],
            &[
                PacketWord::new("hello").unwrap(),
                PacketWord::new("world").unwrap(),
            ]
        );
        let mut out =  vec![0u8; packet_bytes.len()];
        write_packet(&mut out[..], &packet).unwrap();
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
        let seq = PacketSequence::from_raw([0b0000_00000, 0b0000_0000, 0b0000_0000, 0b1000_0000]);
        assert_eq!(seq.origin(), PacketOrigin::Client);
        assert_eq!(seq.number(), 0u32);
        assert_eq!(seq.kind(), PacketKind::Request);
    }

    #[test]
    fn server_packet_sequence_test() {
        let seq = PacketSequence::from_raw([0b0000_0000, 0b0000_0000, 0b0000_0000, 0b0100_0000]);
        assert_eq!(seq.origin(), PacketOrigin::Server);
        assert_eq!(seq.kind(), PacketKind::Response);
        assert_eq!(seq.number(), 0u32);
    }
}

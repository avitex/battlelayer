use std::convert::{TryFrom, TryInto};
use std::str;

use super::packet;
use bytes::Bytes;

#[derive(Debug, Clone)]
pub struct Body {
    words: Vec<Word>,
}

impl Body {
    pub fn new<I, W>(words: I) -> Result<Self, InvalidWordCharError>
    where
        I: IntoIterator<Item = W>,
        W: TryInto<Word, Error = InvalidWordCharError>,
    {
        let mut body_words = Vec::new();
        for word in words.into_iter() {
            match word.try_into() {
                Ok(word) => body_words.push(word),
                Err(invalid_char) => return Err(invalid_char),
            }
        }
        Ok(Self { words: body_words })
    }

    pub fn words(&self) -> &[Word] {
        self.words.as_ref()
    }

    pub fn to_vec(self) -> Vec<Word> {
        self.words
    }
}

impl From<&[Word]> for Body {
    fn from(words: &[Word]) -> Body {
        Self {
            words: words.into(),
        }
    }
}

impl From<Vec<Word>> for Body {
    fn from(words: Vec<Word>) -> Body {
        Self { words }
    }
}

///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, PartialEq)]
pub struct InvalidWordCharError(pub u8);

/// A unit of transmission.
#[derive(Debug, PartialEq, Clone)]
pub struct Word {
    bytes: Bytes,
}

impl Word {
    /// Create a word from a UTF8 string.
    pub fn new(word: &str) -> Result<Self, InvalidWordCharError> {
        Self::from_bytes(Bytes::from(word.as_bytes()))
    }

    /// Create a word from bytes.
    pub fn from_bytes(bytes: Bytes) -> Result<Self, InvalidWordCharError> {
        if let Some(invalid_char) = bytes.as_ref().iter().find(|b| !Self::is_valid_char(**b)) {
            Err(InvalidWordCharError(*invalid_char))
        } else {
            Ok(Self { bytes })
        }
    }

    pub fn is_valid_char(byte: u8) -> bool {
        packet::is_valid_word_char(byte)
    }

    /// Get the UTF8 representation of the word.
    pub fn as_str(&self) -> &str {
        // Safe as we validate each character on contruction.
        unsafe { str::from_utf8_unchecked(self.as_ref()) }
    }

    /// Consume the word as bytes.
    pub fn into_bytes(self) -> Bytes {
        self.bytes
    }

    /// Total size of the word content.
    pub fn byte_size(&self) -> usize {
        self.bytes.len()
    }
}

impl AsRef<[u8]> for Word {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_ref()
    }
}

impl TryFrom<&str> for Word {
    type Error = InvalidWordCharError;

    fn try_from(s: &str) -> Result<Self, InvalidWordCharError> {
        Self::new(s)
    }
}

impl TryFrom<String> for Word {
    type Error = InvalidWordCharError;

    fn try_from(s: String) -> Result<Self, InvalidWordCharError> {
        Self::from_bytes(Bytes::from(s))
    }
}

impl TryFrom<&[u8]> for Word {
    type Error = InvalidWordCharError;

    fn try_from(s: &[u8]) -> Result<Self, InvalidWordCharError> {
        Self::from_bytes(Bytes::from(s))
    }
}

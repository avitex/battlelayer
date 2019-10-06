use std::convert::{TryFrom, TryInto};
use std::{str, fmt};

use super::packet;
use bytes::Bytes;

#[derive(Debug, PartialEq)]
pub enum BodyError {
    InvalidWordChar(u8),
}

#[derive(Debug, Clone)]
pub struct Body {
    content: Vec<Word>,
}

impl Body {
    pub fn new<I, W>(words: I) -> Result<Self, BodyError>
    where
        I: IntoIterator<Item = W>,
        W: TryInto<Word, Error = BodyError>,
    {
        let mut content = Vec::new();
        for word in words.into_iter() {
            match word.try_into() {
                Ok(word) => content.push(word),
                Err(invalid_char) => return Err(invalid_char),
            }
        }
        Ok(Self { content })
    }

    pub fn words(&self) -> &[Word] {
        self.content.as_ref()
    }

    pub fn to_vec(self) -> Vec<Word> {
        self.content
    }
}

impl fmt::Display for Body {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let word_count = self.content.len();
        for i in 0..word_count  {
            write!(fmt, "{}", self.content[i].as_str())?;
            if i < (word_count - 1) {
                write!(fmt, "; ")?;
            }
        }
        return Ok(())
    }
}

impl TryFrom<Vec<String>> for Body {
    type Error = BodyError;

    fn try_from(words: Vec<String>) -> Result<Body, Self::Error> {
        Self::new(words)
    }
}

impl TryFrom<Vec<&str>> for Body {
    type Error = BodyError;

    fn try_from(words: Vec<&str>) -> Result<Body, Self::Error> {
        Self::new(words)
    }
}

impl TryFrom<&[&str]> for Body {
    type Error = BodyError;

    fn try_from(words: &[&str]) -> Result<Body, Self::Error> {
        Self::new(words.to_vec())
    }
}

impl From<&[Word]> for Body {
    fn from(words: &[Word]) -> Body {
        Self {
            content: words.into(),
        }
    }
}

impl From<Vec<Word>> for Body {
    fn from(content: Vec<Word>) -> Body {
        Self { content }
    }
}

///////////////////////////////////////////////////////////////////////////////

/// A unit of transmission.
#[derive(Debug, PartialEq, Clone)]
pub struct Word {
    bytes: Bytes,
}

impl Word {
    /// Create a word from a UTF8 string.
    pub fn new(word: &str) -> Result<Self, BodyError> {
        Self::from_bytes(Bytes::from(word.as_bytes()))
    }

    /// Create a word from bytes.
    pub fn from_bytes(bytes: Bytes) -> Result<Self, BodyError> {
        if let Some(invalid_char) = bytes.as_ref().iter().find(|b| !Self::is_valid_char(**b)) {
            Err(BodyError::InvalidWordChar(*invalid_char))
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
    type Error = BodyError;

    fn try_from(s: &str) -> Result<Self, BodyError> {
        Self::new(s)
    }
}

impl TryFrom<String> for Word {
    type Error = BodyError;

    fn try_from(s: String) -> Result<Self, BodyError> {
        Self::from_bytes(Bytes::from(s))
    }
}

impl TryFrom<&[u8]> for Word {
    type Error = BodyError;

    fn try_from(s: &[u8]) -> Result<Self, BodyError> {
        Self::from_bytes(Bytes::from(s))
    }
}

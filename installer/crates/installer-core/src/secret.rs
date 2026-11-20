use std::fmt;

use zeroize::Zeroize;

pub struct Secret {
    text: String,
}

const CAPACITY: usize = 512;

impl Secret {
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: String::with_capacity(CAPACITY),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn chars(&self) -> usize {
        self.text.chars().count()
    }

    pub fn insert(&mut self, at: usize, ch: char) -> bool {
        if self.text.len() + ch.len_utf8() > CAPACITY {
            return false;
        }
        let byte = byte_index(&self.text, at);
        self.text.insert(byte, ch);
        true
    }

    pub fn remove_before(&mut self, at: usize) {
        if at == 0 {
            return;
        }
        let byte = byte_index(&self.text, at - 1);
        self.text.remove(byte);
    }
}

#[must_use]
pub fn byte_index(text: &str, at: usize) -> usize {
    text.char_indices()
        .nth(at)
        .map_or(text.len(), |(index, _)| index)
}

impl Default for Secret {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Secret {
    fn clone(&self) -> Self {
        let mut copy = Self::new();
        copy.text.push_str(&self.text);
        copy
    }
}

impl From<&str> for Secret {
    fn from(text: &str) -> Self {
        let mut secret = Self::new();
        for ch in text.chars() {
            if secret.text.len() + ch.len_utf8() > CAPACITY {
                break;
            }
            secret.text.push(ch);
        }
        secret
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.text.zeroize();
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Secret").finish_non_exhaustive()
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for Secret {}

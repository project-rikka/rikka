use crate::error::DecodeError;

pub trait Codec: Sized {
    type Error: From<DecodeError>;

    fn encode(&self, out: &mut Writer<'_>);
    fn decode(input: &mut Reader<'_>) -> Result<Self, Self::Error>;
}

#[derive(Debug)]
pub struct Writer<'a> {
    out: &'a mut Vec<u8>,
}

impl<'a> Writer<'a> {
    pub fn new(out: &'a mut Vec<u8>) -> Writer<'a> {
        Writer { out }
    }

    pub fn put<T: Codec>(&mut self, value: &T) {
        value.encode(self);
    }

    pub fn put_all<T: Codec>(&mut self, values: &[T]) {
        for value in values {
            value.encode(self);
        }
    }

    pub fn tag(&mut self, tag: u8) {
        self.out.push(tag);
    }

    pub fn raw(&mut self, bytes: &[u8]) {
        self.out.extend_from_slice(bytes);
    }

    pub fn count32(&mut self, count: usize) {
        let count = u32::try_from(count).expect("list length fits in u32");
        self.put(&count);
    }

    pub fn count64(&mut self, count: usize) {
        self.put(&(count as u64));
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        self.count32(bytes.len());
        self.raw(bytes);
    }

    pub fn str(&mut self, text: &str) {
        self.bytes(text.as_bytes());
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Reader<'a> {
    input: &'a [u8],
}

impl<'a> Reader<'a> {
    #[must_use]
    pub fn new(input: &'a [u8]) -> Reader<'a> {
        Reader { input }
    }

    pub fn get<T: Codec>(&mut self) -> Result<T, T::Error> {
        T::decode(self)
    }

    pub fn tag(&mut self) -> Result<u8, DecodeError> {
        self.get()
    }

    pub fn take(&mut self, count: usize) -> Result<&'a [u8], DecodeError> {
        let (head, rest) = self
            .input
            .split_at_checked(count)
            .ok_or(DecodeError::Malformed("unexpected end of input"))?;
        self.input = rest;
        Ok(head)
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let bytes = self.take(N)?;
        Ok(bytes.try_into().expect("take returns the requested length"))
    }

    pub fn count32(&mut self, least_bytes: usize) -> Result<usize, DecodeError> {
        let count = self.get::<u32>()?;
        self.fits(u64::from(count), least_bytes)
    }

    pub fn count64(&mut self, least_bytes: usize) -> Result<usize, DecodeError> {
        let count = self.get::<u64>()?;
        self.fits(count, least_bytes)
    }

    pub fn bytes(&mut self) -> Result<&'a [u8], DecodeError> {
        let count = self.count32(1)?;
        self.take(count)
    }

    pub fn str(&mut self) -> Result<&'a str, DecodeError> {
        std::str::from_utf8(self.bytes()?).map_err(|_| DecodeError::Malformed("text is not UTF-8"))
    }

    #[must_use]
    pub fn rest(&self) -> &'a [u8] {
        self.input
    }

    pub fn finish(self) -> Result<(), DecodeError> {
        match self.input.is_empty() {
            true => Ok(()),
            false => Err(DecodeError::Malformed("trailing bytes after the end")),
        }
    }

    fn fits(&self, count: u64, least_bytes: usize) -> Result<usize, DecodeError> {
        usize::try_from(count)
            .ok()
            .filter(|&count| count <= self.input.len() / least_bytes.max(1))
            .ok_or(DecodeError::Malformed("list length exceeds input"))
    }
}

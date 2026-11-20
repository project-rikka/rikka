use super::error::Error;
use super::value::Value;

const MAX_DEPTH: usize = 32;

pub(super) fn read(bytes: &[u8]) -> Result<(Value, usize), Error> {
    let mut reader = Reader { bytes, at: 0 };
    let value = reader.value(0)?;
    Ok((value, reader.at))
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn value(&mut self, depth: usize) -> Result<Value, Error> {
        if depth > MAX_DEPTH {
            return Err(Error::TooDeep(MAX_DEPTH));
        }

        let initial = self.byte()?;
        let major = initial >> 5;
        let info = initial & 0x1f;

        if major == 7 {
            return self.simple(initial, info);
        }

        let argument = self.argument(initial, info)?;
        match major {
            0 => Ok(Value::Unsigned(argument)),
            1 => Ok(Value::Negative(argument)),
            2 => Ok(Value::Bytes(self.take(argument)?.to_vec())),
            3 => String::from_utf8(self.take(argument)?.to_vec())
                .map(Value::Text)
                .map_err(|_| Error::Utf8),
            4 => {
                let count = self.count(argument)?;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(self.value(depth + 1)?);
                }
                Ok(Value::Array(items))
            }
            5 => {
                let count = self.count(argument)?;
                let mut entries: Vec<(Value, Value)> = Vec::with_capacity(count);
                for _ in 0..count {
                    let key = self.value(depth + 1)?;
                    if entries.iter().any(|(seen, _)| *seen == key) {
                        return Err(Error::DuplicateKey);
                    }
                    let value = self.value(depth + 1)?;
                    entries.push((key, value));
                }
                Ok(Value::Map(entries))
            }
            _ => Ok(Value::Tag(argument, Box::new(self.value(depth + 1)?))),
        }
    }

    fn simple(&mut self, initial: u8, info: u8) -> Result<Value, Error> {
        match info {
            20 => Ok(Value::Bool(false)),
            21 => Ok(Value::Bool(true)),
            22 => Ok(Value::Null),
            23 => Ok(Value::Undefined),
            24 => Err(Error::Simple(self.byte()?)),
            25 => {
                let raw = self.take(2)?;
                Ok(Value::Float(half(u16::from_be_bytes([raw[0], raw[1]]))))
            }
            26 => {
                let raw: [u8; 4] = self.take(4)?.try_into().map_err(|_| Error::Truncated)?;
                Ok(Value::Float(f64::from(f32::from_be_bytes(raw))))
            }
            27 => {
                let raw: [u8; 8] = self.take(8)?.try_into().map_err(|_| Error::Truncated)?;
                Ok(Value::Float(f64::from_be_bytes(raw)))
            }
            28..=31 => Err(Error::Malformed(initial)),
            simple => Err(Error::Simple(simple)),
        }
    }

    fn argument(&mut self, initial: u8, info: u8) -> Result<u64, Error> {
        match info {
            0..=23 => Ok(u64::from(info)),
            24 => Ok(u64::from(self.byte()?)),
            25 => {
                let raw = self.take(2)?;
                Ok(u64::from(u16::from_be_bytes([raw[0], raw[1]])))
            }
            26 => {
                let raw: [u8; 4] = self.take(4)?.try_into().map_err(|_| Error::Truncated)?;
                Ok(u64::from(u32::from_be_bytes(raw)))
            }
            27 => {
                let raw: [u8; 8] = self.take(8)?.try_into().map_err(|_| Error::Truncated)?;
                Ok(u64::from_be_bytes(raw))
            }
            31 => Err(Error::Indefinite),
            _ => Err(Error::Malformed(initial)),
        }
    }

    fn count(&self, argument: u64) -> Result<usize, Error> {
        let remaining = self.bytes.len() - self.at;
        usize::try_from(argument)
            .ok()
            .filter(|count| *count <= remaining)
            .ok_or(Error::Truncated)
    }

    fn byte(&mut self) -> Result<u8, Error> {
        let byte = *self.bytes.get(self.at).ok_or(Error::Truncated)?;
        self.at += 1;
        Ok(byte)
    }

    fn take(&mut self, length: u64) -> Result<&[u8], Error> {
        let length = self.count(length)?;
        let taken = &self.bytes[self.at..self.at + length];
        self.at += length;
        Ok(taken)
    }
}

fn half(bits: u16) -> f64 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((bits >> 10) & 0x1f);
    let fraction = f64::from(bits & 0x03ff);
    let magnitude = match exponent {
        0 => fraction * 2f64.powi(-24),
        31 if fraction == 0.0 => f64::INFINITY,
        31 => f64::NAN,
        _ => (1.0 + fraction / 1024.0) * 2f64.powi(exponent - 15),
    };
    sign * magnitude
}

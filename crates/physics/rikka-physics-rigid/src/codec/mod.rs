mod body;
mod joint;
mod shape;

use rikka_file_format::{Codec, Reader, Writer};

use crate::{Error, Result};

fn put_list<T: Codec>(out: &mut Writer<'_>, values: &[T]) {
    out.count32(values.len());
    out.put_all(values);
}

fn get_list<T: Codec>(input: &mut Reader<'_>) -> Result<Vec<T>, T::Error> {
    let count = input.count32(1)?;
    (0..count).map(|_| input.get()).collect()
}

fn to_bytes<T: Codec>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    Writer::new(&mut bytes).put(value);
    bytes
}

fn from_bytes<T: Codec<Error = Error>>(bytes: &[u8]) -> Result<T> {
    let mut input = Reader::new(bytes);
    let value = input.get::<T>()?;
    input.finish()?;
    Ok(value)
}

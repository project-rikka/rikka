use std::io::{self, Read};

const MAX_INCOMING: usize = 64 << 20;

pub(crate) const MAX_OUTGOING: usize = 1 << 20;

const HEADER: usize = 4;

pub(crate) fn read(stream: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0; HEADER];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let length = u32::from_ne_bytes(header) as usize;
    if length > MAX_INCOMING {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message of {length} bytes exceeds the limit"),
        ));
    }
    let mut body = vec![0; length];
    stream.read_exact(&mut body)?;
    Ok(Some(body))
}

pub(crate) fn encode(body: &[u8]) -> Option<Vec<u8>> {
    if body.len() > MAX_OUTGOING {
        return None;
    }
    let length = u32::try_from(body.len()).ok()?;
    let mut frame = Vec::with_capacity(HEADER + body.len());
    frame.extend_from_slice(&length.to_ne_bytes());
    frame.extend_from_slice(body);
    Some(frame)
}

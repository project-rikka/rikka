#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Described {
    pub(crate) starts: bool,
    pub(crate) keyframe: bool,
    pub(crate) skip: usize,
}

pub(crate) fn assemble(
    payloads: &[&[u8]],
    describe: fn(&[u8]) -> Option<Described>,
) -> Option<Vec<u8>> {
    let mut frame = Vec::with_capacity(payloads.iter().map(|payload| payload.len()).sum());
    for payload in payloads {
        let described = describe(payload)?;
        frame.extend_from_slice(&payload[described.skip..]);
    }
    Some(frame)
}

use crate::starts::start_codes;

const H264_FILLER: u8 = 12;
const H265_FILLER: u8 = 38;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NalSyntax {
    H264,
    H265,
}

impl NalSyntax {
    fn is_filler(self, header: u8) -> bool {
        match self {
            NalSyntax::H264 => header & 0x1F == H264_FILLER,
            NalSyntax::H265 => (header >> 1) & 0x3F == H265_FILLER,
        }
    }
}

pub fn append_without_filler(stream: &[u8], syntax: NalSyntax, out: &mut Vec<u8>) {
    let starts = start_codes(stream);
    let Some(&(first, _)) = starts.first() else {
        out.extend_from_slice(stream);
        return;
    };
    out.extend_from_slice(&stream[..first]);
    for (index, &(start, header)) in starts.iter().enumerate() {
        let end = starts.get(index + 1).map_or(stream.len(), |next| next.0);
        let filler = stream
            .get(header)
            .is_some_and(|byte| syntax.is_filler(*byte));
        if !filler {
            out.extend_from_slice(&stream[start..end]);
        }
    }
}

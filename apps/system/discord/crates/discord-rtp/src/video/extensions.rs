const TRANSPORT_SEQUENCE: u8 = 5;
const PLAYOUT_DELAY: u8 = 6;
const CONTENT_TYPE: u8 = 7;
const STREAM_ID: u8 = 11;
const REPAIRED_STREAM_ID: u8 = 12;

const SCREENSHARE: u8 = 1;

pub const STREAM_RID: &str = "100";

pub(crate) fn video(transport: u16, repaired: bool) -> Vec<u8> {
    let mut elements = Vec::with_capacity(16);
    element(&mut elements, TRANSPORT_SEQUENCE, &transport.to_be_bytes());
    element(&mut elements, PLAYOUT_DELAY, &[0, 0, 0]);
    element(&mut elements, CONTENT_TYPE, &[SCREENSHARE]);
    let rid = if repaired {
        REPAIRED_STREAM_ID
    } else {
        STREAM_ID
    };
    element(&mut elements, rid, STREAM_RID.as_bytes());
    elements.resize(elements.len().next_multiple_of(4), 0);
    elements
}

fn element(into: &mut Vec<u8>, id: u8, data: &[u8]) {
    debug_assert!((1..=14).contains(&id) && (1..=16).contains(&data.len()));
    into.push(id << 4 | (data.len() as u8 - 1));
    into.extend_from_slice(data);
}

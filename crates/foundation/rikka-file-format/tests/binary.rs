use glam::{Quat, Vec3};
use rikka_file_format::{BinaryFormat, Codec, DecodeError, Reader, Writer};

const TRACE: BinaryFormat = BinaryFormat::new("trace", *b"RKTRACE\0");

#[derive(Debug, PartialEq)]
struct Trace {
    name: String,
    points: Vec<Vec3>,
    turn: Option<Quat>,
}

impl Codec for Trace {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.str(&self.name);
        out.put(&self.points);
        out.put(&self.turn);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Trace {
            name: input.str()?.to_owned(),
            points: input.get()?,
            turn: input.get()?,
        })
    }
}

fn trace() -> Trace {
    Trace {
        name: "walk".into(),
        points: vec![Vec3::X, Vec3::NEG_Z],
        turn: Some(Quat::IDENTITY),
    }
}

#[test]
fn value_round_trips_behind_header() {
    let bytes = TRACE.encode(&trace());
    assert_eq!(bytes[..12], *b"RKTRACE\0\x01\0\0\0");
    assert_eq!(TRACE.decode::<Trace>(&bytes), Ok(trace()));
}

#[test]
fn damaged_input_is_rejected() {
    let bytes = TRACE.encode(&trace());
    let mut other = bytes.clone();
    other[0] = b'X';
    let mut newer = bytes.clone();
    newer[8] = 2;
    let mut extra = bytes.clone();
    extra.push(0);
    let foreign = DecodeError::Foreign { expected: "trace" };
    let cases = [
        (other, foreign),
        (bytes[..5].to_vec(), foreign),
        (
            newer,
            DecodeError::Version {
                format: "trace",
                found: 2,
            },
        ),
        (
            extra,
            DecodeError::Malformed("trailing bytes after the end"),
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(TRACE.decode::<Trace>(&input), Err(expected));
    }
    for cut in 12..bytes.len() {
        assert!(
            TRACE.decode::<Trace>(&bytes[..cut]).is_err(),
            "cut at {cut}"
        );
    }
}

#[test]
fn counts_beyond_input_are_rejected() {
    let mut bytes = Vec::new();
    let mut out = Writer::new(&mut bytes);
    out.put(&0x0102_0304u32);
    out.count32(3);
    assert_eq!(bytes[..4], [4, 3, 2, 1]);
    let mut input = Reader::new(&bytes);
    assert_eq!(input.get::<u32>(), Ok(0x0102_0304));
    assert_eq!(
        input.count32(1),
        Err(DecodeError::Malformed("list length exceeds input"))
    );
    assert!(Reader::new(&[2]).get::<bool>().is_err());
    assert!(
        Reader::new(&[9, 0, 0, 0, 0, 0, 0, 0])
            .get::<Vec<u8>>()
            .is_err()
    );
    assert!(Reader::new(&[1, 0, 0, 0, 0xFF]).str().is_err());
}

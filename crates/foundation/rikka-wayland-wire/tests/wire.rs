use rustix::fd::AsFd;
use std::io::Write;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;

use rikka_wayland_wire::{
    Arg, ArgType, Connection, Fill, Flush, INCOMING_INITIAL, MAX_FDS_PER_MESSAGE, Signature,
    WireError,
};
use rustix::fs::{MemfdFlags, memfd_create};

const EVERYTHING: Signature = &[
    ArgType::Int,
    ArgType::Uint,
    ArgType::Fixed,
    ArgType::String,
    ArgType::NullableString,
    ArgType::Object,
    ArgType::NullableObject,
    ArgType::NewId,
    ArgType::Array,
    ArgType::Fd,
];

fn pair() -> (Connection, UnixStream) {
    let (ours, theirs) = UnixStream::pair().unwrap();
    (Connection::new(ours), theirs)
}

fn connected() -> (Connection, Connection) {
    let (ours, theirs) = UnixStream::pair().unwrap();
    (Connection::new(ours), Connection::new(theirs))
}

fn memfd(marker: u8) -> OwnedFd {
    let fd = memfd_create("wire-test", MemfdFlags::CLOEXEC).unwrap();
    rustix::io::write(&fd, &[marker]).unwrap();
    fd
}

fn marker(fd: &OwnedFd) -> u8 {
    let mut byte = [0u8];
    rustix::io::pread(fd, &mut byte, 0).unwrap();
    byte[0]
}

fn words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_ne_bytes()).collect()
}

fn header(object: u32, opcode: u16, size: usize) -> [u32; 2] {
    [object, ((size as u32) << 16) | u32::from(opcode)]
}

fn raw(stream: &mut UnixStream, body: &[u32]) {
    let mut message = header(7, 3, 8 + body.len() * 4).to_vec();
    message.extend_from_slice(body);
    stream.write_all(&words(&message)).unwrap();
}

fn decode_error(body: &[u32], signature: Signature) -> WireError {
    let (connection, mut stream) = pair();
    raw(&mut stream, body);
    assert!(matches!(connection.fill().unwrap(), Fill::Read(_)));
    let mut message = connection.next_message().unwrap().unwrap();
    let mut args = message.args(signature);
    for arg in args.by_ref() {
        if let Err(error) = arg {
            return error;
        }
    }
    args.finish().unwrap_err()
}

#[test]
fn argument_types_round_trip() {
    let (sender, receiver) = connected();
    let shared = memfd(42);
    let mut writer = sender.write(5, 2);
    writer
        .int(-17)
        .uint(0xDEAD_BEEF)
        .fixed(-2.5)
        .string("héllo")
        .nullable_string(None)
        .object_id(9)
        .object_id(0)
        .object_id(12)
        .array(&[1, 2, 3, 4, 5])
        .fd(shared.as_fd());
    writer.finish().unwrap();
    assert_eq!(sender.flush().unwrap(), Flush::Done);

    assert!(matches!(receiver.fill().unwrap(), Fill::Read(_)));
    let mut message = receiver.next_message().unwrap().unwrap();
    assert_eq!((message.object(), message.opcode()), (5, 2));
    let decoded: Vec<Arg<'_>> = message.args(EVERYTHING).collect::<Result<_, _>>().unwrap();
    assert!(matches!(decoded[0], Arg::Int(-17)));
    assert!(matches!(decoded[1], Arg::Uint(0xDEAD_BEEF)));
    assert!(matches!(decoded[2], Arg::Fixed(value) if value == -2.5));
    assert!(matches!(decoded[3], Arg::String(Some("héllo"))));
    assert!(matches!(decoded[4], Arg::String(None)));
    assert!(matches!(decoded[5], Arg::Object(Some(9))));
    assert!(matches!(decoded[6], Arg::Object(None)));
    assert!(matches!(decoded[7], Arg::NewId(12)));
    assert!(matches!(decoded[8], Arg::Array([1, 2, 3, 4, 5])));
    match &decoded[9] {
        Arg::Fd(fd) => assert_eq!(marker(fd), 42),
        other => panic!("expected an fd, got {other:?}"),
    }
}

#[test]
fn rejects_out_of_bounds_sizes() {
    for size in [4, 6, 4100] {
        let (connection, mut stream) = pair();
        stream.write_all(&words(&header(1, 0, size))).unwrap();
        connection.fill().unwrap();
        assert!(matches!(
            connection.peek(),
            Err(WireError::BadSize(reported)) if reported == size
        ));
    }
}

#[test]
fn rejects_arguments_past_message_end() {
    assert!(matches!(
        decode_error(&[100, 0x6161_6161], &[ArgType::String]),
        WireError::Truncated
    ));
    assert!(matches!(
        decode_error(&[5], &[ArgType::Uint, ArgType::Uint]),
        WireError::Truncated
    ));
    assert!(matches!(
        decode_error(&[64, 1], &[ArgType::Array]),
        WireError::Truncated
    ));
}

#[test]
fn strings_require_utf8_and_nul() {
    let invalid = u32::from_ne_bytes([0xFF, 0xFE, 0x41, 0]);
    assert!(matches!(
        decode_error(&[4, invalid], &[ArgType::String]),
        WireError::NotUtf8
    ));
    let unterminated = u32::from_ne_bytes(*b"abcd");
    assert!(matches!(
        decode_error(&[4, unterminated], &[ArgType::String]),
        WireError::Unterminated
    ));
    assert!(matches!(
        decode_error(&[0], &[ArgType::String]),
        WireError::Null("string")
    ));
}

#[test]
fn excess_fds_fail_read() {
    let (connection, stream) = pair();
    let fds: Vec<OwnedFd> = (0..=MAX_FDS_PER_MESSAGE as u8).map(memfd).collect();
    let borrowed: Vec<_> = fds.iter().map(|fd| fd.as_fd()).collect();
    let mut space = vec![
        std::mem::MaybeUninit::<u8>::uninit();
        rustix::cmsg_space!(ScmRights(MAX_FDS_PER_MESSAGE + 1))
    ];
    let mut control = rustix::net::SendAncillaryBuffer::new(&mut space);
    assert!(control.push(rustix::net::SendAncillaryMessage::ScmRights(&borrowed)));
    let bytes = words(&header(1, 0, 8));
    rustix::net::sendmsg(
        &stream,
        &[std::io::IoSlice::new(&bytes)],
        &mut control,
        rustix::net::SendFlags::empty(),
    )
    .unwrap();
    assert!(matches!(connection.fill(), Err(WireError::TooManyFds)));
}

#[test]
fn fds_stay_ordered_across_batches() {
    let (sender, receiver) = connected();
    let per_message = 20;
    let messages = 4;
    for message in 0..messages {
        let mut writer = sender.write(3, 1);
        for index in 0..per_message {
            writer.fd(memfd((message * per_message + index) as u8).as_fd());
        }
        writer.finish().unwrap();
    }
    assert_eq!(sender.flush().unwrap(), Flush::Done);

    let signature: Vec<ArgType> = vec![ArgType::Fd; per_message];
    let signature: Signature = Box::leak(signature.into_boxed_slice());
    let mut markers = Vec::new();
    while markers.len() < per_message * messages {
        if receiver.peek().unwrap().is_none() {
            assert!(matches!(receiver.fill().unwrap(), Fill::Read(_)));
            continue;
        }
        let mut message = receiver.next_message().unwrap().unwrap();
        for arg in message.args(signature) {
            match arg.unwrap() {
                Arg::Fd(fd) => markers.push(marker(&fd)),
                other => panic!("expected an fd, got {other:?}"),
            }
        }
    }
    let expected: Vec<u8> = (0..(per_message * messages) as u8).collect();
    assert_eq!(markers, expected);
}

#[test]
fn split_messages_decode_intact() {
    let (connection, mut stream) = pair();
    let small = |round: u32| {
        let mut message = header(7, 3, 24).to_vec();
        message.extend([round, round * 3, 4, u32::from_ne_bytes(*b"abc\0")]);
        words(&message)
    };
    let mut stream_bytes = words(&header(7, 3, 4004));
    stream_bytes.extend(vec![0; 3996]);
    for round in 0..40 {
        stream_bytes.extend(small(round));
    }
    let (first, rest) = stream_bytes.split_at(4088);
    stream.write_all(first).unwrap();
    assert_eq!(connection.fill().unwrap(), Fill::Read(4088));
    drop(connection.next_message().unwrap().unwrap());
    stream.write_all(rest).unwrap();

    let mut decoded = 0;
    while decoded < 40 {
        if connection.peek().unwrap().is_none() {
            assert!(matches!(connection.fill().unwrap(), Fill::Read(_)));
            continue;
        }
        let mut message = connection.next_message().unwrap().unwrap();
        let mut args = message.args(&[ArgType::Uint, ArgType::Uint, ArgType::String]);
        assert_eq!(args.uint().unwrap(), decoded);
        assert_eq!(args.uint().unwrap(), decoded * 3);
        assert_eq!(args.string().unwrap(), "abc");
        args.finish().unwrap();
        decoded += 1;
    }
    assert_eq!(connection.incoming_capacity(), INCOMING_INITIAL);
}

#[test]
fn unread_peer_hits_outgoing_limit() {
    let (sender, _silent) = connected();
    let payload = vec![0u8; 4000];
    let mut queued_bytes = 0usize;
    loop {
        let mut writer = sender.write(2, 0);
        writer.array(&payload);
        match writer.finish() {
            Ok(()) => queued_bytes += 4012,
            Err(WireError::Backlog) => break,
            Err(error) => panic!("unexpected {error}"),
        }
        if queued_bytes.is_multiple_of(64 * 4012) {
            sender.flush().unwrap();
        }
    }
    assert!(queued_bytes > 4 << 20);
    assert!(sender.pending_output() <= 4 << 20);
    assert_eq!(sender.flush().unwrap(), Flush::Pending);
}

#[test]
fn fixed_rounds_half_away_from_zero() {
    let (sender, receiver) = connected();
    let values = [
        0.5 / 256.0,
        -0.5 / 256.0,
        0.49 / 256.0,
        -1.25,
        2.0e9,
        f64::NAN,
    ];
    let mut writer = sender.write(1, 0);
    for value in values {
        writer.fixed(value);
    }
    writer.finish().unwrap();
    sender.flush().unwrap();
    receiver.fill().unwrap();
    let mut message = receiver.next_message().unwrap().unwrap();
    let words = message.args(&[ArgType::Fixed; 6]).words::<6>().unwrap();
    let raw: Vec<i32> = words.iter().map(|word| word.int()).collect();
    assert_eq!(raw, [1, -1, 0, -320, i32::MAX, 0]);
}

#[test]
fn partial_write_resumes_intact() {
    let (sender, receiver) = connected();
    let payload = vec![0xA5u8; 1000];
    let messages = 3000u32;
    let carries_fd = |sequence: u32| sequence.is_multiple_of(250);
    for sequence in 0..messages {
        let mut writer = sender.write(2, 0);
        writer.uint(sequence).array(&payload);
        if carries_fd(sequence) {
            writer.fd(memfd(sequence as u8));
        }
        writer.finish().unwrap();
    }
    let mut next = 0u32;
    let mut pending = true;
    while next < messages {
        if pending {
            pending = sender.flush().unwrap() == Flush::Pending;
        }
        match receiver.fill().unwrap() {
            Fill::Read(_) | Fill::Full => {}
            Fill::WouldBlock if pending => {}
            other => panic!("the receiver should keep reading, got {other:?}"),
        }
        while let Some(mut message) = receiver.next_message().unwrap() {
            let signature: Signature = if carries_fd(next) {
                &[ArgType::Uint, ArgType::Array, ArgType::Fd]
            } else {
                &[ArgType::Uint, ArgType::Array]
            };
            let mut args = message.args(signature);
            assert_eq!(args.uint().unwrap(), next);
            assert_eq!(args.array().unwrap(), payload.as_slice());
            if carries_fd(next) {
                assert_eq!(marker(&args.fd().unwrap()), next as u8);
            }
            args.finish().unwrap();
            next += 1;
        }
    }
    assert!(!pending);
    assert_eq!(sender.pending_output(), 0);
    assert_eq!(receiver.queued_fds(), 0);
}

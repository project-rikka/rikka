use std::io::{self, ErrorKind, Read, Write};

use zeroize::Zeroizing;

use crate::conversation::Answer;
use crate::secret::Secret;
use crate::step::{Message, MessageKind, Outcome, Prompt, Step};

const MAX_FIELD_LEN: usize = 1 << 16;
const _: () = assert!(MAX_FIELD_LEN <= u32::MAX as usize);

const PROMPT: u8 = 1;
const MESSAGE: u8 = 2;
const FINISHED: u8 = 3;

const INFO: u8 = 0;
const ERROR: u8 = 1;

const GRANTED: u8 = 0;
const DENIED: u8 = 1;
const CANCELLED: u8 = 2;
const FAILED: u8 = 3;

const REPLY: u8 = 1;

pub(crate) fn send_step(output: &mut impl Write, step: &Step) -> io::Result<()> {
    let mut frame = Vec::new();
    match step {
        Step::Prompt(prompt) => {
            frame.push(PROMPT);
            frame.extend_from_slice(&prompt.serial.to_le_bytes());
            frame.push(u8::from(prompt.echo));
            put_text(&mut frame, prompt.text.as_bytes());
        }
        Step::Message(message) => {
            frame.push(MESSAGE);
            frame.push(match message.kind {
                MessageKind::Info => INFO,
                MessageKind::Error => ERROR,
            });
            put_text(&mut frame, message.text.as_bytes());
        }
        Step::Finished(outcome) => {
            frame.push(FINISHED);
            let (tag, reason) = match outcome {
                Outcome::Granted => (GRANTED, ""),
                Outcome::Denied(reason) => (DENIED, reason.as_str()),
                Outcome::Cancelled => (CANCELLED, ""),
                Outcome::Error(reason) => (FAILED, reason.as_str()),
            };
            frame.push(tag);
            put_text(&mut frame, reason.as_bytes());
        }
    }
    output.write_all(&frame)?;
    output.flush()
}

pub(crate) fn receive_step(input: &mut impl Read) -> io::Result<Option<Step>> {
    let Some(tag) = tag(input)? else {
        return Ok(None);
    };
    let step = match tag {
        PROMPT => {
            let serial = u64::from_le_bytes(bytes(input)?);
            let echo = match byte(input)? {
                0 => false,
                1 => true,
                other => return Err(malformed(format!("echo flag {other}"))),
            };
            Step::Prompt(Prompt {
                serial,
                text: text(input)?,
                echo,
            })
        }
        MESSAGE => {
            let kind = match byte(input)? {
                INFO => MessageKind::Info,
                ERROR => MessageKind::Error,
                other => return Err(malformed(format!("message kind {other}"))),
            };
            Step::Message(Message {
                kind,
                text: text(input)?,
            })
        }
        FINISHED => {
            let tag = byte(input)?;
            let reason = text(input)?;
            Step::Finished(match tag {
                GRANTED => Outcome::Granted,
                DENIED => Outcome::Denied(reason),
                CANCELLED => Outcome::Cancelled,
                FAILED => Outcome::Error(reason),
                other => return Err(malformed(format!("outcome tag {other}"))),
            })
        }
        other => return Err(malformed(format!("step {other}"))),
    };
    Ok(Some(step))
}

pub(crate) fn send_reply(output: &mut impl Write, serial: u64, secret: &Secret) -> io::Result<()> {
    let secret = secret.reveal().as_bytes();
    if secret.len() > MAX_FIELD_LEN {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("a reply longer than {MAX_FIELD_LEN} bytes"),
        ));
    }
    let mut frame = Zeroizing::new(Vec::with_capacity(1 + 8 + 4 + secret.len()));
    frame.push(REPLY);
    frame.extend_from_slice(&serial.to_le_bytes());
    put_text(&mut frame, secret);
    output.write_all(&frame)?;
    output.flush()
}

pub(crate) fn receive_reply(input: &mut impl Read) -> io::Result<Option<Answer>> {
    match tag(input)? {
        None => Ok(None),
        Some(REPLY) => {
            let serial = u64::from_le_bytes(bytes(input)?);
            let length = length(input)?;
            let mut secret = Zeroizing::new(vec![0; length]);
            input.read_exact(&mut secret)?;
            let secret = std::str::from_utf8(&secret)
                .map_err(|_| malformed("a reply that is not UTF-8".to_owned()))?;
            Ok(Some(Answer {
                serial,
                secret: Secret::new(secret.to_owned()),
            }))
        }
        Some(other) => Err(malformed(format!("answer {other}"))),
    }
}

fn put_text(frame: &mut Vec<u8>, text: &[u8]) {
    let text = &text[..text.len().min(MAX_FIELD_LEN)];
    frame.extend_from_slice(&(text.len() as u32).to_le_bytes());
    frame.extend_from_slice(text);
}

fn tag(input: &mut impl Read) -> io::Result<Option<u8>> {
    let mut tag = [0];
    loop {
        match input.read(&mut tag) {
            Ok(0) => return Ok(None),
            Ok(_) => return Ok(Some(tag[0])),
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

fn byte(input: &mut impl Read) -> io::Result<u8> {
    Ok(bytes::<1>(input)?[0])
}

fn bytes<const N: usize>(input: &mut impl Read) -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    input.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn length(input: &mut impl Read) -> io::Result<usize> {
    let length = usize::try_from(u32::from_le_bytes(bytes(input)?)).unwrap_or(usize::MAX);
    match length <= MAX_FIELD_LEN {
        true => Ok(length),
        false => Err(malformed(format!("a text of {length} bytes"))),
    }
}

fn text(input: &mut impl Read) -> io::Result<String> {
    let mut text = vec![0; length(input)?];
    input.read_exact(&mut text)?;
    Ok(String::from_utf8_lossy(&text).into_owned())
}

fn malformed(what: String) -> io::Error {
    io::Error::new(
        ErrorKind::InvalidData,
        format!("malformed PAM helper message: {what}"),
    )
}

use remote_access::clean_file_name;
use rikka_remote_wire::{FileHeader, InputEvent, Parsed, ServerMessage, StreamKind};

use crate::media::{InputCommand, InputHandle};
use crate::notices::Notices;
use crate::uploads;
use crate::viewer::control::ControlWriter;

const CHUNK: usize = 64 * 1024;
const LONGEST_HEADER: usize = 64 * 1024;

pub(crate) async fn accept(
    connection: quinn::Connection,
    input: InputHandle,
    writer: ControlWriter,
    notices: Notices,
) {
    loop {
        let stream = match connection.accept_uni().await {
            Ok(stream) => stream,
            Err(error) => {
                tracing::debug!("no more streams from the viewer: {error}");
                return;
            }
        };
        tokio::spawn(serve(
            stream,
            input.clone(),
            writer.clone(),
            notices.clone(),
        ));
    }
}

async fn serve(
    mut stream: quinn::RecvStream,
    input: InputHandle,
    writer: ControlWriter,
    notices: Notices,
) {
    let mut kind = [0_u8; 1];
    if let Err(error) = stream.read_exact(&mut kind).await {
        tracing::debug!("a stream closed before saying what it carries: {error}");
        return;
    }
    match StreamKind::from_wire(kind[0]) {
        Ok(StreamKind::Input) => events(stream, input).await,
        Ok(StreamKind::File) => upload(stream, writer, &notices).await,
        Err(error) => {
            tracing::debug!("refused a stream: {error}");
            if let Err(error) = stream.stop(0_u32.into()) {
                tracing::debug!("cannot stop the stream: {error}");
            }
        }
    }
}

async fn events(mut stream: quinn::RecvStream, input: InputHandle) {
    let mut buffer = Vec::new();
    loop {
        match stream.read_chunk(CHUNK, true).await {
            Ok(Some(chunk)) => buffer.extend_from_slice(&chunk.bytes),
            Ok(None) => return,
            Err(error) => {
                tracing::debug!("the input stream broke: {error}");
                return;
            }
        }
        let mut start = 0;
        loop {
            match InputEvent::parse(&buffer[start..]) {
                Ok(Parsed::Complete { value, consumed }) => {
                    start += consumed;
                    input.send(InputCommand::Event(value));
                }
                Ok(Parsed::Incomplete) => break,
                Err(error) => {
                    tracing::warn!("the viewer sent malformed input: {error}");
                    return;
                }
            }
        }
        buffer.drain(..start);
    }
}

async fn upload(mut stream: quinn::RecvStream, writer: ControlWriter, notices: &Notices) {
    let mut buffer = Vec::new();
    let (header, consumed) = loop {
        match FileHeader::parse(&buffer) {
            Ok(Parsed::Complete { value, consumed }) => break (value, consumed),
            Ok(Parsed::Incomplete) if buffer.len() < LONGEST_HEADER => {}
            Ok(Parsed::Incomplete) | Err(_) => {
                tracing::warn!("the viewer sent a malformed file header");
                return;
            }
        }
        match stream.read_chunk(CHUNK, true).await {
            Ok(Some(chunk)) => buffer.extend_from_slice(&chunk.bytes),
            Ok(None) => return,
            Err(error) => {
                tracing::debug!("an upload broke off: {error}");
                return;
            }
        }
    };
    let name = clean_file_name(&header.name);
    match uploads::receive(&name, header.size, &buffer[consumed..], &mut stream).await {
        Ok(received) => {
            notices.received(&received);
            writer.send(ServerMessage::FileSaved {
                id: header.id,
                name: received.name,
            });
        }
        Err(error) => {
            tracing::warn!(name, "cannot receive a file: {error}");
            writer.send(ServerMessage::FileFailed {
                id: header.id,
                reason: error.to_string(),
            });
        }
    }
}

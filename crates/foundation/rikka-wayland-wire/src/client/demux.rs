use smallvec::SmallVec;

use crate::client::connection::{Connection, Io};
use crate::client::error::{Error, ProtocolError};
use crate::client::handle::Handle;
use crate::client::packet::{Entry, Packet};
use crate::client::route::Destination;
use crate::client::table::Life;
use crate::error::{Result, WireError};
use crate::ids::DISPLAY_ID;
use crate::interface::{ArgType, MessageSpec};
use crate::message::Header;

const DISPLAY_ERROR: u16 = 0;
const DISPLAY_DELETE_ID: u16 = 1;

pub(crate) enum Arrival {
    Deliver(Destination, Packet),
    Fault(Error),
    Nothing,
}

pub(crate) fn next(connection: &Connection, io: &mut Io) -> Result<Option<Arrival>> {
    let Some(header) = io.connection.peek()? else {
        return Ok(None);
    };
    if header.object == DISPLAY_ID {
        return display(io, header).map(Some);
    }
    let Some(slot) = io.table.get(header.object) else {
        tracing::debug!("event for unknown object {}", header.object);
        drop(io.connection.message(header));
        return Ok(Some(Arrival::Nothing));
    };
    let (interface, generation, version, life) =
        (slot.interface, slot.generation, slot.version, slot.life);
    let route = slot.route.clone();
    let keys = slot.keys.clone();
    let spec = interface
        .events
        .get(usize::from(header.opcode))
        .ok_or(WireError::UnknownOpcode(header.opcode))?;
    let (body, fds) = {
        let mut message = io.connection.message(header);
        let fds = message.take_fds(fd_count(spec))?;
        (Box::<[u8]>::from(message.body()), fds)
    };
    let entries = resolve(io, spec, &body, version, &route, life)?;
    let handle =
        Handle::new(connection, header.object, generation, interface, version).keyed(keys.as_ref());
    if spec.destructor
        && let Some(slot) = io.table.get_mut(header.object)
        && slot.life == Life::Live
    {
        slot.life = Life::Departed;
    }
    if life == Life::Destroyed {
        return Ok(Some(Arrival::Nothing));
    }
    let packet = Packet {
        handle,
        opcode: header.opcode,
        body,
        fds,
        entries,
    };
    Ok(Some(Arrival::Deliver(route, packet)))
}

fn display(io: &mut Io, header: Header) -> Result<Arrival> {
    let body = Box::<[u8]>::from(io.connection.message(header).body());
    let word = |index: usize| -> Result<u32> {
        let bytes = body
            .get(index * 4..index * 4 + 4)
            .ok_or(WireError::Truncated)?;
        Ok(u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    match header.opcode {
        DISPLAY_ERROR => {
            let (object, code, length) = (word(0)?, word(1)?, word(2)? as usize);
            let text = body
                .get(12..12 + length.saturating_sub(1))
                .ok_or(WireError::Truncated)?;
            Ok(Arrival::Fault(Error::Protocol(ProtocolError {
                interface: io
                    .table
                    .get(object)
                    .map_or("unknown", |slot| slot.interface.name),
                object,
                code,
                message: String::from_utf8_lossy(text).into_owned(),
            })))
        }
        DISPLAY_DELETE_ID => {
            io.table.release(word(0)?);
            Ok(Arrival::Nothing)
        }
        opcode => Err(WireError::UnknownOpcode(opcode)),
    }
}

fn fd_count(spec: &MessageSpec) -> usize {
    spec.signature
        .iter()
        .filter(|kind| **kind == ArgType::Fd)
        .count()
}

fn resolve(
    io: &mut Io,
    spec: &MessageSpec,
    body: &[u8],
    version: u32,
    route: &Destination,
    life: Life,
) -> Result<SmallVec<[Entry; 2]>> {
    let mut entries = SmallVec::new();
    let mut offset = 0;
    let mut children = spec.children.iter();
    let word = |offset: usize| -> Result<u32> {
        let bytes = body.get(offset..offset + 4).ok_or(WireError::Truncated)?;
        Ok(u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    for kind in spec.signature {
        match kind {
            ArgType::Int | ArgType::Uint | ArgType::Fixed => offset += 4,
            ArgType::Fd => {}
            ArgType::String | ArgType::NullableString | ArgType::Array => {
                let length = word(offset)? as usize;
                offset += 4 + (length.checked_add(3).ok_or(WireError::Truncated)? & !3);
            }
            ArgType::Object | ArgType::NullableObject => {
                let id = word(offset)?;
                offset += 4;
                if let Some(slot) = io.table.get(id) {
                    entries.push(Entry {
                        id,
                        generation: slot.generation,
                        interface: slot.interface,
                        version: slot.version,
                    });
                }
            }
            ArgType::NewId => {
                let id = word(offset)?;
                offset += 4;
                let interface = children.next().ok_or(WireError::InvalidNewId(id))?;
                let generation = io
                    .table
                    .adopt(id, interface, version, route.clone())
                    .ok_or(WireError::InvalidNewId(id))?;
                if life == Life::Destroyed
                    && let Some(slot) = io.table.get_mut(id)
                {
                    slot.life = Life::Destroyed;
                }
                entries.push(Entry {
                    id,
                    generation,
                    interface,
                    version,
                });
            }
            ArgType::UntypedNewId => return Err(WireError::InvalidNewId(0)),
        }
    }
    Ok(entries)
}

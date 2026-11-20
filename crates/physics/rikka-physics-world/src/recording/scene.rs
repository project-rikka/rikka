use rikka_file_format::{Codec, Reader, Writer};
use rikka_physics_rigid::{Body, JointDef, ShapeDef};

use crate::history::Setup;
use crate::{Error, Result};

impl Codec for Setup {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        match self {
            Setup::Body(body) => {
                out.tag(0);
                out.put(body);
            }
            Setup::Shape(body, shape) => {
                out.tag(1);
                out.put(body);
                out.put(shape);
            }
            Setup::Joint(joint) => {
                out.tag(2);
                out.put(joint.as_ref());
            }
            Setup::Character(def) => {
                out.tag(3);
                out.put(def);
            }
        }
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        match input.tag()? {
            0 => Ok(Setup::Body(input.get::<Body>().map_err(|_| {
                Error::MalformedRecording("malformed body definition")
            })?)),
            1 => {
                let body = input.get()?;
                let shape = input
                    .get::<ShapeDef>()
                    .map_err(|_| Error::MalformedRecording("malformed shape definition"))?;
                Ok(Setup::Shape(body, shape))
            }
            2 => Ok(Setup::Joint(Box::new(input.get::<JointDef>().map_err(
                |_| Error::MalformedRecording("malformed joint definition"),
            )?))),
            3 => Ok(Setup::Character(input.get()?)),
            _ => Err(Error::MalformedRecording("unknown scene entry")),
        }
    }
}

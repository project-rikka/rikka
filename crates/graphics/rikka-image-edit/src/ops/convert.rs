use std::sync::Arc;

use crate::color::{Conversion, ConversionKind, Intent, Profile, Space};
use crate::error::Result;
use crate::ops::Effect;
use crate::picture::shown::ConvertPush;
use crate::picture::{Frame, Mode};
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

#[derive(Clone, Debug, PartialEq)]
pub struct ConvertProfile {
    to: Profile,
    intent: Intent,
}

impl ConvertProfile {
    #[must_use]
    pub fn new(to: Profile) -> ConvertProfile {
        ConvertProfile {
            to,
            intent: Intent::default(),
        }
    }

    #[must_use]
    pub fn intent(mut self, intent: Intent) -> ConvertProfile {
        self.intent = intent;
        self
    }
}

impl Effect for ConvertProfile {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let space = work.imaging().space(Some(&self.to));
        let converted = converted(work, frame, space, self.intent)?;
        Ok(Frame {
            mode: match self.to.is_gray() {
                true => Mode::Gray,
                false => Mode::Rgb,
            },
            ..converted
        })
    }
}

pub(crate) fn converted(
    work: &mut Work,
    frame: &Frame,
    space: Arc<Space>,
    intent: Intent,
) -> Result<Frame> {
    let conversion = Conversion::new(frame.space.profile(), space.profile(), intent)?;
    if conversion.kind == ConversionKind::Identity {
        return Ok(Frame {
            space,
            ..frame.clone()
        });
    }
    let prepared = conversion.prepare(work)?;
    let out = work.plane(frame.extent(), frame.depth.working())?;
    let push = ConvertPush {
        head: PushHeader::new(&frame.plane, &out)
            .mode(prepared.kind as u32)
            .clamped(frame.depth.clamps()),
        data: prepared.data.address(),
        cube: prepared.cube,
        premultiply: 0,
        from: [0, 0],
    };
    work.pass(
        Shader::Convert,
        push,
        Io::new(&frame.plane, &out).read_buffer(&prepared.data),
        8.0,
    )?;
    Ok(Frame {
        plane: out,
        space,
        ..frame.clone()
    })
}

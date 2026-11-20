use std::ops::RangeInclusive;

use libspa::param::ParamType;
use libspa::pod::Pod;
use libspa::utils::result::AsyncSeq;

use crate::format::SampleFormat;
use crate::hardware::Hardware;

use super::{Session, alsa};

pub(super) struct FormatQuery {
    pending: AsyncSeq,
    formats: Vec<SampleFormat>,
    rates: Option<RangeInclusive<u32>>,
    available: bool,
    requery: bool,
}

impl Session {
    pub(super) fn query_formats(&mut self, id: u32) {
        let Some(watched) = self.nodes.get_mut(&id) else {
            return;
        };
        if let Some(query) = &mut watched.format_query {
            query.requery = true;
            return;
        }
        watched
            .proxy
            .enum_params(0, Some(ParamType::EnumFormat), 0, u32::MAX);
        let Ok(pending) = self.core.sync(0) else {
            return;
        };
        watched.format_query = Some(FormatQuery {
            pending,
            formats: Vec::new(),
            rates: None,
            available: false,
            requery: false,
        });
    }

    pub(super) fn record_format(&mut self, id: u32, pod: &Pod) {
        let Some(query) = self
            .nodes
            .get_mut(&id)
            .and_then(|watched| watched.format_query.as_mut())
        else {
            return;
        };
        let Some(offer) = alsa::offer(pod) else {
            return;
        };
        query.available = true;
        query.formats.extend(offer.formats);
        query.rates = match (query.rates.take(), offer.rates) {
            (Some(current), Some(available)) => {
                Some(*current.start().min(available.start())..=*current.end().max(available.end()))
            }
            (current, available) => current.or(available),
        };
    }

    pub(super) fn finish_format_query(&mut self, pending: AsyncSeq) {
        let Some((&id, watched)) = self.nodes.iter_mut().find(|(_, watched)| {
            watched
                .format_query
                .as_ref()
                .is_some_and(|query| query.pending == pending)
        }) else {
            return;
        };
        let Some(query) = watched.format_query.take() else {
            return;
        };
        let pins = watched.pins.unwrap_or_default();
        let observed = Hardware {
            formats: if pins.format {
                Vec::new()
            } else {
                query.formats
            },
            rates: if pins.rate { None } else { query.rates },
            refuses_pinned_format: false,
        };
        let hardware = Hardware {
            refuses_pinned_format: pins.format && !query.available,
            ..self.learnt.learn(&watched.node.name, &observed)
        };
        let moved = watched.node.hardware.as_ref() != Some(&hardware);
        watched.node.hardware = Some(hardware);
        self.publish_if(moved);
        if query.requery {
            self.query_formats(id);
        }
    }
}

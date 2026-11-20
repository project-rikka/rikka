use crate::reading;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Load {
    pub one: f32,
    pub five: f32,
    pub fifteen: f32,
}

pub(crate) fn read() -> Load {
    reading::text("/proc/loadavg")
        .and_then(|text| parse(&text))
        .unwrap_or_default()
}

fn parse(text: &str) -> Option<Load> {
    let mut fields = text.split_whitespace().map(str::parse::<f32>);
    Some(Load {
        one: fields.next()?.ok()?,
        five: fields.next()?.ok()?,
        fifteen: fields.next()?.ok()?,
    })
}

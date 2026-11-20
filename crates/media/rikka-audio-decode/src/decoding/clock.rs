#[derive(Clone, Copy)]
pub(crate) struct Clock {
    numer: i128,
    denom: i128,
    rate: i128,
}

impl Clock {
    pub(crate) fn new(numer: i64, denom: i64, rate: u32) -> Clock {
        Clock {
            numer: i128::from(numer.max(1)),
            denom: i128::from(denom.max(1)),
            rate: i128::from(rate.max(1)),
        }
    }

    pub(crate) fn exact(&self) -> bool {
        self.numer == 1 && self.denom == self.rate
    }

    pub(crate) fn whole(&self) -> bool {
        self.denom % (self.numer * self.rate) == 0
    }

    pub(crate) fn frames(&self, ticks: i64) -> i64 {
        if self.exact() {
            return ticks;
        }
        narrow(round_div(
            i128::from(ticks) * self.numer * self.rate,
            self.denom,
        ))
    }

    pub(crate) fn ticks(&self, frames: i64) -> i64 {
        if self.exact() {
            return frames;
        }
        narrow(round_div(
            i128::from(frames) * self.denom,
            self.numer * self.rate,
        ))
    }
}

fn narrow(value: i128) -> i64 {
    i64::try_from(value).unwrap_or(if value < 0 { i64::MIN } else { i64::MAX })
}

fn round_div(value: i128, by: i128) -> i128 {
    let half = by / 2;
    match value >= 0 {
        true => (value + half) / by,
        false => (value - half) / by,
    }
}

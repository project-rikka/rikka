use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use futures_util::{Stream, StreamExt};
use rikka_location::{Accuracy, Fix, Location};
use rikka_worker::{Observable, WallAlarm};
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;

use crate::air::Air;
use crate::cache::{self, Cache, CacheEntry};
use crate::error::{Error, Result};
use crate::fetch::{Fetcher, parsed};
use crate::nominatim;
use crate::open_meteo::{air, forecast};
use crate::place::{Place, Spot, rounded};
use crate::report::Report;
use crate::schedule::{AIR_LIFE, FORECAST_LIFE, Schedule};
use crate::weather::{Online, Order, State, WeatherError};

const MOVED: f64 = 5_000.0;
const UNLOCATED: Duration = Duration::from_secs(30);
const HERE: &str = "here";

type Fixes = Pin<Box<dyn Stream<Item = Option<Fix>> + Send>>;

pub(crate) struct Forecaster {
    app: String,
    fetcher: Fetcher,
    cache: Cache,
    state: Observable<State>,
    location: Option<Location>,
    fixes: Option<Fixes>,
    looking_since: Option<Instant>,
    schedule: Schedule,
    air: Option<CacheEntry>,
    alarm: AsyncFd<WallAlarm>,
    armed: Option<SystemTime>,
    online: Option<Online>,
    reachable: Option<bool>,
}

impl Forecaster {
    pub(crate) fn new(
        app: String,
        state: Observable<State>,
        alarm: AsyncFd<WallAlarm>,
        online: Option<Online>,
    ) -> Result<Forecaster> {
        Ok(Forecaster {
            app,
            fetcher: Fetcher::new()?,
            cache: Cache::open(),
            state,
            location: None,
            fixes: None,
            looking_since: None,
            schedule: Schedule::default(),
            air: None,
            alarm,
            armed: None,
            online,
            reachable: None,
        })
    }

    pub(crate) async fn run(mut self, mut orders: mpsc::UnboundedReceiver<Order>, spot: Spot) {
        self.aim(spot).await;
        loop {
            if self.place().is_some()
                && self.reachable != Some(false)
                && self.schedule.due(SystemTime::now())
            {
                self.refresh().await;
            }
            self.arm();
            tokio::select! {
                order = orders.recv() => match order {
                    Some(Order::Spot(spot)) => self.aim(spot).await,
                    Some(Order::Refresh) => self.schedule.soon(),
                    Some(Order::Stop) | None => return,
                },
                fix = next_fix(&mut self.fixes) => self.located(fix).await,
                reachable = next_reach(&mut self.online) => self.set_reachable(reachable),
                () = rang(&self.alarm) => self.armed = None,
                () = lost(self.looking_since) => self.notice_unlocated(),
            }
        }
    }

    fn arm(&mut self) {
        let needed = match self.place().is_some() && self.reachable != Some(false) {
            true => self.schedule.next(),
            false => None,
        };
        if needed == self.armed {
            return;
        }
        let alarm = self.alarm.get_ref();
        let armed = match needed {
            Some(at) => alarm.arm(at),
            None => alarm.disarm(),
        };
        match armed {
            Ok(()) => self.armed = needed,
            Err(error) => tracing::warn!("failed to schedule next forecast: {error}"),
        }
    }

    fn set_reachable(&mut self, reachable: Option<bool>) {
        let Some(reachable) = reachable else {
            self.online = None;
            self.reachable = None;
            return;
        };
        let regained = reachable && self.reachable != Some(true);
        self.reachable = Some(reachable);
        if regained && self.schedule.failing() {
            self.schedule.soon();
        }
    }

    fn place(&self) -> Option<Place> {
        self.state.with(|state| state.place.clone())
    }

    fn publish(&self, change: impl FnOnce(&mut State)) {
        self.state.modify(|state| {
            let before = state.clone();
            change(state);
            *state != before
        });
    }

    async fn aim(&mut self, spot: Spot) {
        self.schedule.soon();
        self.air = None;
        match spot.clone() {
            Spot::Fixed(place) => {
                self.location = None;
                self.fixes = None;
                self.looking_since = None;
                self.publish(|state| {
                    state.spot = spot;
                    state.error = None;
                });
                self.settle_at(place);
            }
            Spot::Here => {
                let remembered = self
                    .cache
                    .read(HERE)
                    .and_then(|entry| serde_json::from_str::<Place>(&entry.body).ok());
                self.publish(|state| {
                    state.spot = spot;
                    state.place = None;
                    state.report = None;
                    state.error = None;
                });
                if let Some(place) = remembered {
                    self.settle_at(place);
                }
                let location = Location::start(self.app.clone(), Accuracy::City);
                let current = location.state();
                self.fixes = Some(Box::pin(location.changes()));
                self.location = Some(location);
                self.looking_since = Some(Instant::now());
                if current.is_some() {
                    self.located(current).await;
                }
            }
        }
    }

    fn settle_at(&mut self, place: Place) {
        let report = self.cached_report(&place);
        if let Some(report) = &report {
            self.schedule.fetched(report.fetched_at);
        }
        self.publish(|state| {
            state.place = Some(place);
            state.report = report.map(Arc::new);
        });
    }

    async fn located(&mut self, fix: Option<Fix>) {
        let Some(fix) = fix else {
            return;
        };
        self.looking_since = None;
        let unlocated = self
            .state
            .with(|state| state.error == Some(WeatherError::NoLocation));
        if unlocated {
            self.publish(|state| state.error = None);
        }
        let near = self
            .place()
            .is_some_and(|place| place.metres_to(fix.latitude, fix.longitude) < MOVED);
        if near {
            return;
        }
        let place = self.named(&fix).await;
        self.cache.write(
            HERE,
            &CacheEntry::new(
                SystemTime::now(),
                serde_json::to_string(&place).unwrap_or_default(),
            ),
        );
        self.schedule.soon();
        self.air = None;
        self.settle_at(place);
    }

    async fn named(&self, fix: &Fix) -> Place {
        let (latitude, longitude) = (rounded(fix.latitude), rounded(fix.longitude));
        let language = rikka_i18n::current_locale().language().to_owned();
        let key = format!("{}-{language}", cache::key("place", latitude, longitude));
        let body = match self.cache.read(&key) {
            Some(entry) => Ok(entry.body),
            None => self
                .fetcher
                .reverse(latitude, longitude, &language)
                .await
                .inspect(|body| {
                    self.cache
                        .write(&key, &CacheEntry::new(SystemTime::now(), body.clone()))
                }),
        };
        body.and_then(|body| parsed::<nominatim::Answer>(&body))
            .inspect_err(|error| tracing::debug!("reverse geocoding failed: {error}"))
            .ok()
            .and_then(|answer| answer.into_place(latitude, longitude))
            .unwrap_or_else(|| Place {
                name: fix.description.clone().unwrap_or_default(),
                region: None,
                country: None,
                country_code: None,
                latitude,
                longitude,
            })
    }

    fn notice_unlocated(&mut self) {
        self.looking_since = None;
        if self.state.with(|state| state.error.is_none()) {
            self.publish(|state| state.error = Some(WeatherError::NoLocation));
        }
    }

    fn cached_report(&self, place: &Place) -> Option<Report> {
        let (latitude, longitude) = (rounded(place.latitude), rounded(place.longitude));
        let entry = self
            .cache
            .read(&cache::key("forecast", latitude, longitude))?;
        let air = self
            .cache
            .read(&cache::key("air", latitude, longitude))
            .and_then(|entry| parsed::<air::Answer>(&entry.body).ok()?.into_air());
        let mut report = parsed::<forecast::Forecast>(&entry.body)
            .ok()?
            .into_report(place.clone(), entry.fetched())?;
        report.air = air;
        Some(report)
    }

    async fn refresh(&mut self) {
        let Some(place) = self.place() else {
            return;
        };
        let now = SystemTime::now();
        match self.fetched(&place, now).await {
            Ok(report) => {
                self.schedule.fetched(report.fetched_at);
                self.publish(|state| {
                    state.refreshing = false;
                    state.error = None;
                    if state.place.as_ref() == Some(&place) {
                        state.report = Some(Arc::new(report));
                    }
                });
            }
            Err(error) => {
                tracing::debug!("failed to fetch weather: {error}");
                self.schedule.failed(now);
                self.publish(|state| {
                    state.refreshing = false;
                    state.error = Some(WeatherError::Service(error));
                });
            }
        }
    }

    async fn fetched(&mut self, place: &Place, now: SystemTime) -> Result<Report> {
        let (latitude, longitude) = (rounded(place.latitude), rounded(place.longitude));
        let key = cache::key("forecast", latitude, longitude);
        let entry = match self.cache.read(&key) {
            Some(entry) if entry.younger_than(FORECAST_LIFE, now) => entry,
            _ => {
                self.publish(|state| state.refreshing = true);
                let body = self.fetcher.forecast(latitude, longitude).await?;
                let entry = CacheEntry::new(now, body);
                self.cache.write(&key, &entry);
                entry
            }
        };
        let mut report = parsed::<forecast::Forecast>(&entry.body)?
            .into_report(place.clone(), entry.fetched())
            .ok_or_else(|| Error::Unreadable("the forecast has no current conditions".into()))?;
        report.air = self.air(latitude, longitude, now).await;
        Ok(report)
    }

    async fn air(&mut self, latitude: f64, longitude: f64, now: SystemTime) -> Option<Air> {
        let key = cache::key("air", latitude, longitude);
        let fresh = |entry: &CacheEntry| entry.younger_than(AIR_LIFE, now);
        if !self.air.as_ref().is_some_and(fresh) {
            self.air = match self.cache.read(&key) {
                Some(entry) if fresh(&entry) => Some(entry),
                cached => match self.fetcher.air(latitude, longitude).await {
                    Ok(body) => {
                        let entry = CacheEntry::new(now, body);
                        self.cache.write(&key, &entry);
                        Some(entry)
                    }
                    Err(error) => {
                        tracing::debug!("failed to fetch air quality: {error}");
                        cached
                    }
                },
            };
        }
        parsed::<air::Answer>(&self.air.as_ref()?.body)
            .ok()?
            .into_air()
    }
}

async fn next_fix(fixes: &mut Option<Fixes>) -> Option<Fix> {
    if let Some(stream) = fixes
        && let Some(fix) = stream.next().await
    {
        return fix;
    }
    std::future::pending().await
}

async fn next_reach(online: &mut Option<Online>) -> Option<bool> {
    match online {
        Some(stream) => stream.next().await,
        None => std::future::pending().await,
    }
}

async fn rang(alarm: &AsyncFd<WallAlarm>) {
    loop {
        let Ok(mut ready) = alarm.readable().await else {
            return std::future::pending().await;
        };
        let rang = ready.get_inner().read();
        ready.clear_ready();
        match rang {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(error) => {
                tracing::warn!("failed to read system time: {error}");
                return std::future::pending().await;
            }
        }
    }
}

async fn lost(looking_since: Option<Instant>) {
    match looking_since {
        Some(since) => tokio::time::sleep_until((since + UNLOCATED).into()).await,
        None => std::future::pending().await,
    }
}

use rikka_compositor_geometry::{Logical, Point, Transform};
use rikka_compositor_planes::VrrPolicy;
use rikka_compositor_types::OutputId;
use rikka_settings::seam::Settings;
use rikka_settings::session::displays::{
    self as keys, ADAPTIVE_SYNC, AdaptiveSync, CONNECTED, ENABLED, HDR, MODE, MODES, NAME,
    POSITION, Position, SCALE, SCALES, SDR_BRIGHTNESS, SDR_BRIGHTNESS_DEFAULT,
    SDR_BRIGHTNESS_RANGE, TRANSFORM, TRANSFORMS,
};
use rikka_settings::{Key, Schema};

use crate::outputs::{Head, Mode, Output, OutputConfig};

pub(crate) use rikka_settings::session::displays::DOMAIN;

const DENSITY_STEPS: [(f64, f64); 2] = [(192.0, 2.0), (144.0, 1.5)];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    slug: String,
    label: String,
}

const MAX_SLUG: usize = 96;

impl Identity {
    pub(crate) fn new(make: &str, model: &str, serial: Option<&str>, connector: &str) -> Self {
        let described = !is_unknown(make) || !is_unknown(model);
        let mut parts: Vec<&str> = Vec::new();
        if described {
            parts.extend([make, model]);
        }
        match serial.filter(|serial| identifies(serial)) {
            Some(serial) if described => parts.push(serial),
            _ => parts.push(connector),
        }

        let slug = slugify(&parts.join("-"));
        let label = if described {
            format!("{} {model} ({connector})", make.trim())
        } else {
            connector.to_owned()
        };
        Self {
            slug: if slug.is_empty() {
                slugify(connector)
            } else {
                slug
            },
            label,
        }
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn slug(&self) -> &str {
        &self.slug
    }

    fn key(&self, suffix: &str) -> String {
        keys::key(&self.slug, suffix)
    }

    fn disambiguate(&mut self, connector: &str) {
        self.slug = slugify(&format!("{}-{connector}", self.slug));
    }
}

fn is_unknown(field: &str) -> bool {
    field.trim().is_empty() || field.eq_ignore_ascii_case("unknown")
}

fn identifies(serial: &str) -> bool {
    let meaningful = serial
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    !meaningful.is_empty()
        && !meaningful.bytes().all(|byte| byte == b'0')
        && !meaningful.eq_ignore_ascii_case("na")
        && !meaningful.eq_ignore_ascii_case("unknown")
}

fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    while slug.len() > MAX_SLUG {
        slug.pop();
    }
    slug.trim_matches('-').to_owned()
}

#[derive(Clone, Debug)]
pub(crate) struct Display {
    id: OutputId,
    identity: Identity,
    modes: Vec<Mode>,
    hdr_capable: bool,
    auto: OutputConfig,
}

impl Display {
    pub(crate) fn new(id: OutputId, head: &Head) -> Option<Self> {
        let identity = Identity::new(&head.make, &head.model, head.serial.as_deref(), &head.name);

        let mut modes = head.modes.clone();
        modes.sort_by_key(|mode| {
            std::cmp::Reverse((u64::from(mode.width) * u64::from(mode.height), mode.refresh))
        });
        modes.dedup();
        let best = *modes.first()?;

        let scale = if head.size_mm.0 > 0 {
            automatic_scale(head.size_mm.0, best.width)
        } else {
            head.scale.clamp(*SCALES.start(), *SCALES.end())
        };

        Some(Self {
            id,
            identity,
            auto: OutputConfig {
                enabled: true,
                mode: best,
                scale,
                transform: Transform::Normal,
                position: Point::zero(),
                adaptive_sync: VrrPolicy::Automatic,
                hdr: false,
                sdr_brightness: SDR_BRIGHTNESS_DEFAULT,
            },
            modes,
            hdr_capable: head.hdr.is_some(),
        })
    }

    pub(crate) fn placed(mut self, position: Point<i32, Logical>) -> Self {
        self.auto.position = position;
        self
    }

    pub(crate) fn automatic(&self) -> OutputConfig {
        self.auto
    }

    fn automatic_width(&self) -> i32 {
        (f64::from(self.auto.mode.width) / self.auto.scale).round() as i32
    }

    pub(crate) fn identity(&self) -> &Identity {
        &self.identity
    }

    pub(crate) fn id(&self) -> OutputId {
        self.id
    }

    fn keys(&self) -> impl Iterator<Item = Key> {
        let identity = &self.identity;
        let auto = self.auto;
        let modes: Vec<String> = self.modes.iter().map(|mode| text(*mode)).collect();
        let hdr = self.hdr_capable.then(|| {
            [
                Key::bool(identity.key(HDR), auto.hdr)
                    .summary("High dynamic range and wide colour output"),
                Key::float(identity.key(SDR_BRIGHTNESS), auto.sdr_brightness)
                    .summary("Brightness of standard content in HDR (cd/m²)")
                    .range(SDR_BRIGHTNESS_RANGE),
            ]
        });

        [
            Key::string(identity.key(NAME), identity.label()).summary("Display name (read-only)"),
            Key::string(identity.key(MODES), keys::listed(&modes))
                .summary("Supported resolutions and refresh rates (read-only)"),
            Key::bool(identity.key(ENABLED), auto.enabled)
                .summary("Whether the display is enabled"),
            Key::string(identity.key(MODE), text(auto.mode))
                .summary("Resolution and refresh rate")
                .choices(modes),
            Key::string(identity.key(POSITION), placed(auto.position).to_string())
                .summary("Top-left position in the layout (`x,y`)"),
            Key::float(identity.key(SCALE), auto.scale)
                .summary("Scale factor")
                .range(SCALES),
            Key::string(
                identity.key(TRANSFORM),
                keys::transform_name(auto.transform),
            )
            .summary("Rotation and reflection")
            .choices(TRANSFORMS),
            Key::choice::<AdaptiveSync>(identity.key(ADAPTIVE_SYNC)).summary(
                "Variable refresh rate: never, while a fullscreen window shows moving content, or always",
            ),
        ]
        .into_iter()
        .chain(hdr.into_iter().flatten())
    }

    pub(crate) fn config(&self, settings: &Settings) -> OutputConfig {
        let identity = &self.identity;
        let auto = self.auto;
        OutputConfig {
            enabled: settings.read_or(&identity.key(ENABLED), auto.enabled),
            mode: settings
                .read::<String>(&identity.key(MODE))
                .and_then(|text| self.mode_named(&text))
                .unwrap_or(auto.mode),
            scale: settings.read_or(&identity.key(SCALE), auto.scale),
            transform: settings
                .read::<String>(&identity.key(TRANSFORM))
                .and_then(|text| keys::transform_named(&text))
                .unwrap_or(auto.transform),
            position: settings
                .read::<String>(&identity.key(POSITION))
                .and_then(|text| Position::parse(&text))
                .map(|position| Point::new(position.x, position.y))
                .unwrap_or(auto.position),
            adaptive_sync: vrr_policy(settings.choice(&identity.key(ADAPTIVE_SYNC))),
            hdr: self.hdr_capable && settings.read_or(&identity.key(HDR), auto.hdr),
            sdr_brightness: match self.hdr_capable {
                true => settings
                    .read_or(&identity.key(SDR_BRIGHTNESS), auto.sdr_brightness)
                    .clamp(*SDR_BRIGHTNESS_RANGE.start(), *SDR_BRIGHTNESS_RANGE.end()),
                false => auto.sdr_brightness,
            },
        }
    }

    fn mode_named(&self, named: &str) -> Option<Mode> {
        let named = keys::Mode::parse(named)?;
        self.modes
            .iter()
            .copied()
            .find(|mode| listed(*mode) == named)
    }
}

fn vrr_policy(adaptive_sync: AdaptiveSync) -> VrrPolicy {
    match adaptive_sync {
        AdaptiveSync::Never => VrrPolicy::Never,
        AdaptiveSync::Automatic => VrrPolicy::Automatic,
        AdaptiveSync::Always => VrrPolicy::Always,
    }
}

fn automatic_scale(width_mm: u32, width_px: u32) -> f64 {
    if width_mm == 0 || width_px == 0 {
        return 1.0;
    }
    let dpi = f64::from(width_px) / (f64::from(width_mm) / 25.4);
    DENSITY_STEPS
        .iter()
        .find(|(threshold, _)| dpi >= *threshold)
        .map_or(1.0, |(_, scale)| *scale)
}

fn listed(mode: Mode) -> keys::Mode {
    keys::Mode {
        width: mode.width,
        height: mode.height,
        refresh: mode.refresh,
    }
}

fn text(mode: Mode) -> String {
    listed(mode).to_string()
}

fn placed(position: Point<i32, Logical>) -> Position {
    Position {
        x: position.x,
        y: position.y,
    }
}

pub(crate) fn describe<'a>(outputs: impl IntoIterator<Item = &'a Output>) -> Vec<Display> {
    let mut outputs: Vec<&Output> = outputs.into_iter().collect();
    outputs.sort_by(|one, other| port(one.name()).cmp(&port(other.name())));

    let mut displays: Vec<Display> = Vec::with_capacity(outputs.len());
    let mut x = 0;
    for output in outputs {
        let Some(mut display) = Display::new(output.id(), output.head()) else {
            continue;
        };
        if displays
            .iter()
            .any(|other| other.identity().slug() == display.identity().slug())
        {
            display.identity.disambiguate(output.name());
        }
        let width = display.automatic_width();
        displays.push(display.placed(Point::new(x, 0)));
        x += width;
    }
    displays
}

fn port(name: &str) -> (&str, u32) {
    match name.rsplit_once('-') {
        Some((interface, number)) => (interface, number.parse().unwrap_or(u32::MAX)),
        None => (name, 0),
    }
}

pub(crate) fn schema(displays: &[Display]) -> Schema {
    let connected = keys::listed(displays.iter().map(|display| display.identity().slug()));

    let mut schema = Schema::new(DOMAIN)
        .summary("Connected displays and their arrangement")
        .key(
            Key::string(CONNECTED, connected)
                .summary("Connected displays (read-only, `;`-separated)"),
        );
    for key in displays.iter().flat_map(Display::keys) {
        schema = schema.key(key);
    }
    schema
}

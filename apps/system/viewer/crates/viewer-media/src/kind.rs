use rikka_media_format::Kind as Media;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Picture,
    Sound,
    Movie,
    Model,
    Text,
}

impl From<Media> for Kind {
    fn from(kind: Media) -> Kind {
        match kind {
            Media::Picture => Kind::Picture,
            Media::Sound => Kind::Sound,
            Media::Movie => Kind::Movie,
        }
    }
}

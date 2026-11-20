#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToonSurface {
    #[default]
    Cloth,
    Skin,
    Face,
    Hair,
    Eye,
    Mouth,
    Lash,
    Blush,
    Stocking,
}

const RULES: [(ToonSurface, &[&str]); 10] = [
    (
        ToonSurface::Blush,
        &[
            "頬",
            "チーク",
            "照れ",
            "赤面",
            "ほほ",
            "涙",
            "blush",
            "cheek",
            "tear",
            "tears",
        ],
    ),
    (
        ToonSurface::Face,
        &["髪影", "髮影", "发影", "前髪影", "hairshadow"],
    ),
    (
        ToonSurface::Stocking,
        &[
            "タイツ",
            "ストッキング",
            "パンスト",
            "黒スト",
            "丝袜",
            "絲襪",
            "连裤袜",
            "tights",
            "stocking",
            "pantyhose",
            "hosiery",
        ],
    ),
    (
        ToonSurface::Cloth,
        &[
            "飾",
            "ゴム",
            "ピン",
            "眼鏡",
            "メガネ",
            "めがね",
            "镜",
            "鏡",
            "レンズ",
            "glasses",
            "lens",
            "ornament",
            "accessory",
            "clip",
            "ribbon",
        ],
    ),
    (
        ToonSurface::Lash,
        &[
            "眉", "まゆ", "まつ", "睫", "brow", "eyebrow", "lash", "eyelash", "eyeline",
        ],
    ),
    (
        ToonSurface::Eye,
        &[
            "白目",
            "目",
            "眼",
            "瞳",
            "ハイライト",
            "eye",
            "iris",
            "pupil",
            "sclera",
        ],
    ),
    (
        ToonSurface::Mouth,
        &[
            "^口", "口内", "口腔", "歯", "舌", "mouth", "teeth", "tooth", "tongue",
        ],
    ),
    (ToonSurface::Face, &["顔", "颜", "フェイス", "face"]),
    (
        ToonSurface::Hair,
        &[
            "髪",
            "髮",
            "发",
            "アホ毛",
            "もみあげ",
            "hair",
            "bangs",
            "ponytail",
        ],
    ),
    (
        ToonSurface::Skin,
        &[
            "肌", "皮膚", "素体", "身体", "体", "首", "耳", "skin", "body", "neck", "ear",
        ],
    ),
];

impl ToonSurface {
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        let lowered = name.to_lowercase();
        let words = words(name);
        RULES
            .iter()
            .find(|(_, keys)| keys.iter().any(|key| matches(key, &lowered, &words)))
            .map(|(surface, _)| *surface)
    }

    #[must_use]
    pub fn classify(name: &str, texture: Option<&str>) -> Self {
        Self::named(name)
            .or_else(|| texture.map(stem).and_then(Self::named))
            .unwrap_or_default()
    }
}

fn matches(key: &str, lowered: &str, words: &[String]) -> bool {
    if let Some(prefix) = key.strip_prefix('^') {
        return lowered.starts_with(prefix);
    }
    if key.is_ascii() {
        return words.iter().any(|word| {
            word == key
                || word
                    .strip_suffix('s')
                    .is_some_and(|single| single == key || single.strip_suffix('e') == Some(key))
        });
    }
    lowered.contains(key)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Lower,
    Upper,
    Digit,
}

fn words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous: Option<Kind> = None;
    for character in name.chars() {
        let kind = match character {
            'a'..='z' => Kind::Lower,
            'A'..='Z' => Kind::Upper,
            '0'..='9' => Kind::Digit,
            _ => {
                previous = None;
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
                continue;
            }
        };
        let breaks = matches!(
            (previous, kind),
            (Some(Kind::Lower | Kind::Digit), Kind::Upper)
                | (Some(Kind::Digit), Kind::Lower)
                | (Some(Kind::Lower | Kind::Upper), Kind::Digit)
        );
        if breaks && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        previous = Some(kind);
        current.push(character.to_ascii_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn stem(path: &str) -> &str {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file.rsplit_once('.').map_or(file, |(stem, _)| stem)
}

use std::sync::{Arc, LazyLock};

use animate_body::Body;
use rikka_humanoid::Anatomy;
use rikka_mmd::standard_skeleton;

use super::Performer;

pub(super) static STANDARD: LazyLock<Performer> = LazyLock::new(|| {
    let model = Arc::new(standard_skeleton());
    let body = Body::from_anatomy(&model, &Anatomy::recognize(&model))
        .expect("the standard MMD skeleton maps to a whole humanoid");
    Performer { model, body }
});

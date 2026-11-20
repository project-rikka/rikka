use rikka_thread::Attachment;

use crate::prelude::*;

const TALLEST: f32 = 320.0;
const WIDEST: f32 = 560.0;

pub(crate) fn view(pictures: &[Attachment]) -> Flex {
    row_of(pictures.iter().map(picture))
        .gap(Space::SNUG)
        .align_items(Align::Start)
        .wrap_items()
        .fill_width()
}

fn picture(attachment: &Attachment) -> Element {
    let (width, height) = attachment
        .extent()
        .filter(|(width, height)| *width > 0 && *height > 0)
        .map_or((1.0, 1.0), |(width, height)| (width as f32, height as f32));
    let scale = (TALLEST / height).min(WIDEST / width).min(1.0);

    container(
        image(ImageSource::Bytes(attachment.data().clone()))
            .natural_size(width, height)
            .fit(ImageFit::Contain)
            .radius(Corner::Item)
            .fill(),
    )
    .radius(Corner::Item)
    .border(1.0, palette().outline)
    .width(width * scale)
    .height(height * scale)
    .max_width(Length::FILL)
    .any()
}

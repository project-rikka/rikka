use std::time::{Duration, Instant};

use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Point, Size};
use rikka_ui::*;
use rikka_widgets::*;

const VIEW: Size = Size::new(420.0, 200.0);
const SETTLED: Duration = Duration::from_secs(5);
const INSIDE: Point = Point::new(60.0, 12.0);

#[derive(State)]
struct Desk {
    document: TextDocument,
    attached: Vec<String>,
}

fn desk_view(desk: &Desk) -> Element {
    drop_zone(text_editor(&desk.document).fill())
        .accepts(|offer: &DragOffer| offer.has_uris().then(|| Accept::copy("text/uri-list")))
        .on_drop(|desk: &mut Desk, offer: DragOffer, _: DragAction| {
            desk.attached.extend(offer.uris());
        })
        .size(Length::FILL, Length::FILL)
        .build()
}

struct OfferDelivery(Offer);

impl Delivery for OfferDelivery {
    fn read(&self, mime: &str) -> std::io::Result<Vec<u8>> {
        self.0
            .data(mime)
            .map(|data| data.to_vec())
            .ok_or_else(|| std::io::Error::other("never offered"))
    }
}

fn offered(offer: Offer) -> DragOffer {
    DragOffer::new(offer.mime_types().map(str::to_owned)).delivered_by(OfferDelivery(offer))
}

fn desk(text: &str) -> (TypedUi<Desk>, Scene) {
    let document = TextDocument::new(text);
    document.place_caret(text.len(), false);
    let mut ui = TypedUi::new(
        Desk {
            document,
            attached: Vec::new(),
        },
        desk_view,
        Theme::dark(),
    );
    let mut scene = Scene::new();
    ui.set_viewport(VIEW, 1.0);
    settle(&mut ui, &mut scene);
    (ui, scene)
}

fn settle(ui: &mut TypedUi<Desk>, scene: &mut Scene) {
    let now = Instant::now();
    ui.render(Rendering::new(scene).with_time(now));
    ui.render(Rendering::new(scene).with_time(now + SETTLED));
}

fn dropped(ui: &mut TypedUi<Desk>, scene: &mut Scene, offer: &DragOffer) -> Option<Accept> {
    ui.handle_event(Event::DragMoved {
        position: INSIDE,
        offer: offer.clone(),
    });
    settle(ui, scene);
    let accepted = ui.drag_accepts().cloned();
    if let Some(accept) = &accepted {
        ui.handle_event(Event::DragDropped {
            position: INSIDE,
            offer: offer.clone(),
            action: accept.preferred,
        });
        settle(ui, scene);
    }
    accepted
}

#[test]
fn dropped_text_is_inserted() {
    let (mut ui, mut scene) = desk("");
    let offer = offered(Offer::text("dropped words"));

    let accepted = dropped(&mut ui, &mut scene, &offer).expect("editor rejected text");

    assert_eq!(accepted.mime, "text/plain;charset=utf-8");
    assert_eq!(
        accepted.preferred,
        DragAction::Copy,
        "external text not copied"
    );
    assert_eq!(ui.state().document.text(), "dropped words");
    assert!(ui.state().attached.is_empty());
}

fn hover_view(_: &Desk) -> Element {
    component(|desk: &Desk| desk, hover_zone)
}

fn hover_zone(_: &Desk) -> Element {
    drop_zone(local_state(false, |hovered: bool, set: Setter<bool>| {
        container(text(match hovered {
            true => "hovered",
            false => "resting",
        }))
        .on_hover(move |over: bool| {
            set(over);
            skip_view();
        })
        .size(Length::FILL, Length::FILL)
    }))
    .accepts(|offer: &DragOffer| offer.has_uris().then(|| Accept::copy("text/uri-list")))
    .on_drop(|desk: &mut Desk, offer: DragOffer, _: DragAction| {
        desk.attached.extend(offer.uris());
    })
    .size(Length::FILL, Length::FILL)
    .build()
}

#[test]
fn content_with_its_own_state_updates_inside_drop_zone() {
    let mut ui = TypedUi::new(
        Desk {
            document: TextDocument::new(""),
            attached: Vec::new(),
        },
        hover_view,
        Theme::dark(),
    );
    let mut scene = Scene::new();
    ui.set_viewport(VIEW, 1.0);
    settle(&mut ui, &mut scene);

    ui.handle_event(Event::PointerMoved(INSIDE));
    settle(&mut ui, &mut scene);

    let shown = ui.dump();
    assert!(
        shown.contains("hovered"),
        "content kept stale state:\n{shown}"
    );
}

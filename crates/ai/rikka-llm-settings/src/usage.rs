use crate::prelude::*;
use rikka_llm::{Trouble, Window};
use rikka_llm_accounts::{
    Account, Reading, Readings, UsageReport, open_verification, verification_label,
};
use rikka_usage_bar::usage_bar;
use rikka_widgets::{column, row};

use crate::llm_settings::LlmSettings;
use crate::notice::notice;
use crate::rows::{asking, muted};

#[derive(Debug, Default)]
pub(crate) struct Usage {
    readings: Readings,
}

impl Usage {
    pub(crate) fn reread(&mut self, llm: &rikka_llm_accounts::Llm) {
        for gauging in self.readings.request(llm) {
            spawn(
                gauging.run(),
                |settings: &mut LlmSettings, gauged: UsageReport| {
                    settings.usage.readings.record(gauged);
                },
            );
        }
    }
}

pub(crate) fn view(usage: &Usage, account: &Account) -> Option<Element> {
    if !account.is_metered() {
        return None;
    }

    let reading = usage.readings.get(account.id());
    let windows = reading.map_or(&[] as &[Window], Reading::windows);

    Some(
        column! {
            heading();
            reading.and_then(Reading::error).map(trouble);
            for window in windows {
                usage_bar(window);
            }
            if windows.is_empty() {
                if reading.is_some_and(Reading::is_pending) {
                    asking(tr!("llm-settings.asking"));
                } else {
                    muted(tr!("llm-settings.unmetered"));
                }
            }
        }
        .gap(Space::NORMAL)
        .fill_width()
        .build(),
    )
}

fn trouble(trouble: &Trouble) -> Container {
    let reported = banner(trouble.reason().to_owned()).tint(palette().warning);
    notice(match trouble.verification_url() {
        Some(url) => {
            let url = url.to_owned();
            reported.on_action(verification_label(), move || open_verification(&url))
        }
        None => reported,
    })
}

fn heading() -> impl View {
    row! {
        text(tr!("llm-settings.usage")).label().medium().grow(1.0);
        button(
            row! {
                icon(MaterialSymbol::REFRESH).extent(Length::px(14.0));
                text(tr!("llm-settings.refresh")).caption();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .ghost()
        .small()
        .on_press(|settings: &mut LlmSettings| settings.usage.reread(&settings.llm));
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
}

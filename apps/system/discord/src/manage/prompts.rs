use discord_core::MfaMethod;
use rikka_app::prelude::*;
use rikka_widgets::column;

use super::{Manage, MfaAsk, Prompt};
use crate::chat::Chat;

const WIDTH: f32 = 420.0;

pub(super) fn prompt(prompt: &Prompt, stage: &'static str) -> Vec<Element> {
    confirm_prompt(
        prompt.title.clone(),
        prompt.body.clone(),
        prompt.accept.clone(),
        |chat: &mut Chat, accepted: bool| chat.answer_prompt(accepted),
    )
    .keyed("discord-manage-prompt")
    .centered_on(stage)
    .with_scrim()
}

pub(super) fn alert(message: &str, stage: &'static str) -> Vec<Element> {
    alert_prompt(
        tr!("discord.manage.failed"),
        message.to_owned(),
        |manage: &mut Manage| manage.dismiss_alert(),
    )
    .keyed("discord-manage-alert")
    .centered_on(stage)
    .with_scrim()
}

fn method_label(method: MfaMethod) -> Label {
    match method {
        MfaMethod::Totp => tr!("discord.mfa.totp"),
        MfaMethod::Sms => tr!("discord.mfa.sms"),
        MfaMethod::Backup => tr!("discord.mfa.backup"),
        MfaMethod::Password => tr!("discord.mfa.password"),
    }
}

pub(super) fn mfa(manage: &Manage, ask: &MfaAsk, stage: &'static str) -> Vec<Element> {
    let methods = ask.challenge.methods.clone();
    let chosen = methods
        .iter()
        .position(|method| *method == ask.method)
        .unwrap_or(0);
    let busy = manage.is_busy();
    let sms_waiting = ask.method == MfaMethod::Sms && !ask.sent;
    let body = column! {
        text(tr!("discord.mfa.hint")).caption().muted().fill_width();
        if methods.len() > 1 {
            methods
                .iter()
                .fold(segmented(chosen), |choice, method| choice.segment(method_label(*method)))
                .on_select(move |manage: &mut Manage, at: usize| {
                    if let Some(method) = methods.get(at).copied() {
                        manage.choose_mfa(method);
                    }
                })
                .fill_width();
        }
        if sms_waiting {
            button(text(tr!("discord.mfa.send-sms")))
                .secondary()
                .enabled(!busy)
                .on_press(|chat: &mut Chat| chat.send_mfa_sms());
        }
        text_field(manage.code_field())
            .placeholder(method_label(ask.method))
            .secret(ask.method == MfaMethod::Password)
            .autofocus(true)
            .enabled(!busy && !sms_waiting)
            .error(manage.error_message().is_some())
            .on_change(|manage: &mut Manage| manage.recode())
            .on_submit(|chat: &mut Chat| chat.submit_mfa())
            .fill_width();
        if let Some(error_message) = manage.error_message() {
            text(error_message.to_owned()).caption().danger().fill_width();
        }
    }
    .gap(Space::NORMAL)
    .fill_width();
    dialog(tr!("discord.mfa.title"), body)
        .cancel(tr!("discord.manage.cancel"), |chat: &mut Chat| {
            chat.cancel_mfa()
        })
        .accept(tr!("discord.mfa.verify"), |chat: &mut Chat| {
            chat.submit_mfa()
        })
        .ready(manage.is_coded() && !busy)
        .width(WIDTH)
        .keyed("discord-manage-mfa")
        .centered_on(stage)
        .with_scrim()
}

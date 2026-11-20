use std::time::Duration;

use proton_config::Config;
use proton_session::{Access, AccountStore, ErrorBanner, Failure, Renewed, Resumed, Step};
use rikka_app::prelude::*;
use rikka_proton_api::delay;
use rikka_proton_cache::Cache;
use rikka_proton_client::{Change, Renewal, Session};
use rikka_widgets::{column, row};

use crate::clients::{Clients, Update};
use crate::product::Product;
use crate::signing::{self, Signing, Stage};
use crate::workspace::{Workspace, WorkspaceId};
use crate::{account, codes, drive, mail, pane, pass};

const RAIL: f32 = 86.0;
const RENEWAL: &str = "desk.renewal";
const EVENTS: &str = "desk.events";
const FIRST_RENEWAL_RETRY: Duration = Duration::from_secs(2);
const LAST_RENEWAL_RETRY: Duration = Duration::from_secs(300);

#[derive(Clone, Debug)]
struct Storage {
    access: Access,
    cache: Cache,
}

#[derive(State)]
pub(crate) struct Desk {
    storage: Option<Storage>,
    config: Config,
    product: Product,
    error: ErrorBanner,
    #[state]
    signing: Signing,
    #[state]
    workspace: Option<Workspace>,
}

impl Desk {
    pub(crate) fn new(_sender: Sender<Desk>) -> Desk {
        Desk {
            storage: None,
            config: Config::load(),
            product: Product::default(),
            error: ErrorBanner::default(),
            signing: Signing::new(),
            workspace: None,
        }
    }

    pub(crate) fn start(&mut self) {
        spawn_blocking(proton_files::clear_viewing_dir, |_: &mut Desk, ()| {
            skip_view()
        });
        spawn_blocking(
            open,
            |desk: &mut Desk, storage: Result<Storage, Failure>| {
                desk.ready(storage);
            },
        );
    }

    pub(crate) fn signing(&self) -> &Signing {
        &self.signing
    }

    pub(crate) fn workspace_of(&mut self, workspace_id: WorkspaceId) -> Option<&mut Workspace> {
        self.workspace
            .as_mut()
            .filter(|workspace| workspace.workspace_id() == workspace_id)
    }

    pub(crate) fn command(&mut self, command: Command) {
        let product = self.product;
        match self.workspace.as_mut() {
            Some(workspace) => workspace.command(product, command),
            None => skip_view(),
        }
    }

    pub(crate) fn open(&mut self, product: Product) {
        if self.product == product {
            skip_view();
            return;
        }
        let Some(workspace) = self.workspace.as_mut() else {
            return;
        };
        workspace.rest(self.product);
        self.product = product;
        workspace.opened(product);
    }

    pub(crate) fn sign_in(&mut self) {
        let (Some(access), false) = (self.access(), self.signing.busy) else {
            return;
        };
        let username = self.signing.username.text().trim().to_owned();
        let password = self.signing.password.text();
        if username.is_empty() || password.is_empty() {
            return;
        }

        self.signing.working();
        spawn(
            async move { Ok(access.sign_in(username, password).await?) },
            |desk: &mut Desk, stepped: Result<Step, Failure>| desk.stepped(stepped),
        );
    }

    pub(crate) fn unlock(&mut self) {
        let (Some(access), false) = (self.access(), self.signing.busy) else {
            return;
        };
        let password = self.signing.password.text();
        if password.is_empty() {
            return;
        }

        self.signing.working();
        spawn(
            async move { Ok(Step::Signed(Box::new(access.unlock(password).await?))) },
            |desk: &mut Desk, stepped: Result<Step, Failure>| desk.stepped(stepped),
        );
    }

    pub(crate) fn submit_code(&mut self) {
        let Stage::Code(pending) = self.signing.take() else {
            return;
        };
        let code = self.signing.code.text().trim().to_owned();
        if code.is_empty() {
            self.signing.stage = Stage::Code(pending);
            return;
        }

        self.signing.working();
        spawn(
            async move { Ok(pending.totp(code).await?) },
            |desk: &mut Desk, stepped: Result<Step, Failure>| desk.stepped(stepped),
        );
    }

    pub(crate) fn submit_mailbox(&mut self) {
        let Stage::Mailbox(pending) = self.signing.take() else {
            return;
        };
        let password = self.signing.mailbox.text();
        if password.is_empty() {
            self.signing.stage = Stage::Mailbox(pending);
            return;
        }

        self.signing.working();
        spawn(
            async move { Ok(pending.submit(password).await?) },
            |desk: &mut Desk, stepped: Result<Step, Failure>| desk.stepped(stepped),
        );
    }

    pub(crate) fn sign_out(&mut self) {
        let (Some(access), Some(workspace)) = (self.access(), self.workspace.as_mut()) else {
            return;
        };
        let session = workspace.session().clone();
        workspace.account.leaving();
        spawn(
            async move { Ok(access.sign_out(&session).await?) },
            |desk: &mut Desk, left: Result<(), Failure>| desk.left(left),
        );
    }

    pub(crate) fn keep_key(&mut self, keep: bool) {
        self.config.set_stay_signed_in(keep);
        let Some(access) = self.access() else {
            return;
        };
        access.set_stay_signed_in(keep);
        let Some(session) = self
            .workspace
            .as_ref()
            .map(|workspace| workspace.session().clone())
        else {
            return;
        };
        match keep {
            true => spawn(access.keep_key(&session), |_: &mut Desk, ()| skip_view()),
            false => spawn(access.drop_key(&session), |_: &mut Desk, ()| skip_view()),
        }
    }

    pub(crate) fn set_conversations(&mut self, grouped: bool) {
        self.config.set_conversations(grouped);
        if let Some(workspace) = self.workspace.as_mut() {
            workspace.mail.group(grouped);
        }
    }

    pub(crate) fn set_tiles(&mut self, tiles: bool) {
        self.config.set_drive_tiles(tiles);
        if let Some(workspace) = self.workspace.as_mut() {
            workspace.drive.tile(tiles);
        }
    }

    pub(crate) fn clear_cache(&mut self) {
        if let Some(storage) = self.storage.as_ref() {
            let cache = storage.cache.clone();
            spawn_blocking(
                move || cache.clear(),
                |desk: &mut Desk, cleared: Result<(), rikka_proton_cache::Error>| {
                    if let Err(error) = cleared {
                        desk.error.set(error);
                    }
                },
            );
        }
        if let Some(workspace) = self.workspace.as_mut() {
            workspace.account.forgot();
        }
    }

    pub(crate) fn forget_account(&mut self) {
        if let Some(access) = self.access() {
            spawn(access.forget(), |_: &mut Desk, ()| skip_view());
        }
        self.signing.username.clear();
        self.signing.password.clear();
        self.signing.fresh();
    }

    fn access(&self) -> Option<Access> {
        self.storage.as_ref().map(|storage| storage.access.clone())
    }

    fn ready(&mut self, storage: Result<Storage, Failure>) {
        let Some(storage) = self.error.capture(storage) else {
            self.signing.fresh();
            return;
        };
        storage
            .access
            .set_stay_signed_in(self.config.stay_signed_in());
        let warming = storage.access.clone();
        let access = storage.access.clone();
        self.storage = Some(storage);

        spawn(
            async move { warming.warm_up().await },
            |_: &mut Desk, ()| {
                skip_view();
            },
        );
        spawn(
            async move { Ok(access.resume().await?) },
            |desk: &mut Desk, resumed: Result<Resumed, Failure>| desk.resumed(resumed),
        );
    }

    fn resumed(&mut self, resumed: Result<Resumed, Failure>) {
        match resumed {
            Ok(Resumed::Cached(session)) => {
                self.signed(*session);
                if let Some(workspace_id) = self.workspace.as_ref().map(Workspace::workspace_id) {
                    self.renew(workspace_id, 0);
                }
            }
            Ok(Resumed::SignedIn(session)) => self.signed(*session),
            Ok(Resumed::Locked(username)) => self.signing.lock(username),
            Ok(Resumed::SignedOut) => self.signing.fresh(),
            Err(failure) => {
                self.signing.fresh();
                self.signing.failed(failure);
            }
        }
    }

    fn stepped(&mut self, stepped: Result<Step, Failure>) {
        match stepped {
            Ok(Step::Signed(session)) => self.signed(*session),
            Ok(Step::SecondFactor(pending)) => self.signing.ask_for_code(pending),
            Ok(Step::MailboxPassword(pending)) => self.signing.ask_for_mailbox(pending),
            Err(failure) => self.signing.failed(failure),
        }
    }

    fn signed(&mut self, session: Session) {
        let Some(storage) = self.storage.as_ref() else {
            return;
        };
        self.signing.settled();
        let mut workspace = Workspace::new(session, storage.cache.current_era(), &self.config);
        let workspace_id = workspace.workspace_id();
        Desk::watch(workspace_id, workspace.clients());
        workspace.opened(self.product);
        self.workspace = Some(workspace);
    }

    fn renew(&mut self, workspace_id: WorkspaceId, attempt: u32) {
        let Some(access) = self.access() else {
            return;
        };
        let Some(session) = self
            .workspace_of(workspace_id)
            .map(|workspace| workspace.session().clone())
        else {
            skip_view();
            return;
        };
        spawn(
            access.renew(&session),
            move |desk: &mut Desk, renewed: proton_session::Result<Renewed>| {
                desk.renewed(workspace_id, attempt, renewed);
            },
        );
    }

    fn adopt(&mut self, workspace_id: WorkspaceId, renewal: Renewal) {
        let Some(access) = self.access() else {
            return;
        };
        spawn(
            access.adopt(renewal),
            move |desk: &mut Desk, renewed: Renewed| {
                desk.renewed(workspace_id, 0, Ok(renewed));
            },
        );
    }

    fn renewed(
        &mut self,
        workspace_id: WorkspaceId,
        attempt: u32,
        renewed: proton_session::Result<Renewed>,
    ) {
        if self.workspace_of(workspace_id).is_none() {
            skip_view();
            return;
        }
        match renewed {
            Ok(Renewed::Current) => skip_view(),
            Ok(Renewed::ProfileChanged(session)) => {
                if let Some(workspace) = self.workspace.as_mut() {
                    workspace.describe(*session);
                }
            }
            Ok(Renewed::KeysChanged(session)) => {
                self.close_workspace();
                self.signed(*session);
            }
            Ok(Renewed::SignedOut) => self.left(Ok(())),
            Err(error) if error.is_unreachable() => {
                after_keyed(RENEWAL, renewal_delay(attempt), move |desk: &mut Desk| {
                    desk.renew(workspace_id, attempt + 1)
                });
                skip_view();
            }
            Err(error) => self.error.set(error),
        }
    }

    fn left(&mut self, left: Result<(), Failure>) {
        self.error.capture(left);
        self.close_workspace();
        spawn_blocking(proton_files::clear_viewing_dir, |_: &mut Desk, ()| {
            skip_view()
        });
        self.product = Product::default();
        self.signing = Signing::new();
        self.signing.fresh();
    }

    fn close_workspace(&mut self) {
        cancel(RENEWAL);
        cancel(EVENTS);
        if let Some(mut workspace) = self.workspace.take() {
            workspace.rest(self.product);
        }
    }

    fn watch(workspace_id: WorkspaceId, clients: &Clients) {
        let changes = clients.event_loop().start();
        stream_keyed(
            EVENTS,
            changes,
            move |desk: &mut Desk, change: Change<Update>| {
                desk.changed(workspace_id, change);
            },
        );
    }

    fn changed(&mut self, workspace_id: WorkspaceId, change: Change<Update>) {
        let Some(workspace) = self.workspace_of(workspace_id) else {
            skip_view();
            return;
        };
        match change {
            Change::Product(Update::Mail(event)) => workspace.mail.apply_event(&event),
            Change::Product(Update::Pass(events)) => workspace.pass.apply_events(&events),
            Change::Renewed(renewal) => self.adopt(workspace_id, renewal),
            Change::SignedOut => self.signed_out(workspace_id),
            Change::Refreshed(_) => skip_view(),
        }
    }

    fn signed_out(&mut self, workspace_id: WorkspaceId) {
        let Some(access) = self.access() else {
            skip_view();
            return;
        };
        spawn(access.forget(), move |desk: &mut Desk, ()| {
            match desk.workspace_of(workspace_id) {
                Some(_) => desk.left(Ok(())),
                None => skip_view(),
            }
        });
    }

    fn rail(&self) -> impl View {
        let chosen = self.product;
        row! {
            container(
                column_of(Product::ALL.map(|product| leaf(product, product == chosen).build()))
                    .gap(Space::TIGHT)
                    .align_items(Align::Center)
                    .fill_width(),
            )
            .padding(Inset::xy(6.0, 10.0))
            .width(RAIL)
            .fill_height();
            rule_upright();
        }
        .fill_height()
    }

    fn body(&self, workspace: &Workspace) -> Element {
        match self.product {
            Product::Mail => mail::view(&workspace.mail).build(),
            Product::Drive => drive::view(&workspace.drive).build(),
            Product::Pass => pass::view(&workspace.pass).build(),
            Product::Codes => codes::view(&workspace.codes).build(),
            Product::Account => account::view(&workspace.account, &self.config).build(),
        }
    }
}

fn leaf(product: Product, chosen: bool) -> impl View {
    let tint = match chosen {
        true => palette().accent,
        false => palette().text_muted,
    };

    container(
        column! {
            icon(product.symbol()).extent(Length::px(20.0)).color(tint);
            text(product.title())
                .caption()
                .color(tint)
                .align(TextAlign::Center)
                .no_wrap()
                .fill_width();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width(),
    )
    .background(match chosen {
        true => palette().accent.with_alpha(0.12),
        false => Color::TRANSPARENT,
    })
    .radius(Corner::Panel)
    .padding(Inset::xy(4.0, 10.0))
    .fill_width()
    .on_press(move |desk: &mut Desk| desk.open(product))
}

pub(crate) fn view(desk: &Desk) -> impl View {
    column! {
        pane::error_banner(&desk.error, |desk: &mut Desk| &mut desk.error);
        if let Some(workspace) = &desk.workspace {
            row! {
                desk.rail();
                page_stack(desk.product.at(), desk.body(workspace)).grow(1.0).fill_height();
            }
            .grow(1.0);
        } else {
            signing::view(desk.signing()).grow(1.0);
        }
    }
    .fill()
}

pub(crate) fn caption_bar(desk: &Desk) -> Option<CaptionBar> {
    let user = desk
        .workspace
        .as_ref()
        .map(|workspace| workspace.session().keys().user());
    let title = match user {
        Some(_) => desk.product.title(),
        None => tr!("proton.title"),
    };

    let bar = caption()
        .leading(
            row! {
                icon(MaterialSymbol::SHIELD).extent(Length::px(16.0)).color(palette().accent);
                text(title).label().medium().no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .trailing(
            row! {
                if let Some(user) = user {
                    text(
                        user
                            .email()
                            .or_else(|| user.name())
                            .unwrap_or_default()
                            .to_owned(),
                    )
                    .caption()
                    .muted()
                    .no_wrap();
                }
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        );

    Some(bar.into())
}

fn renewal_delay(attempt: u32) -> Duration {
    delay::doubling(FIRST_RENEWAL_RETRY, attempt, LAST_RENEWAL_RETRY)
}

fn open() -> Result<Storage, Failure> {
    let store = AccountStore::open()?;
    let cache = store.cache();
    let access = Access::open(store)?;
    Ok(Storage { access, cache })
}

impl std::fmt::Debug for Desk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Desk")
            .field("product", &self.product)
            .field("signed", &self.workspace.is_some())
            .finish_non_exhaustive()
    }
}

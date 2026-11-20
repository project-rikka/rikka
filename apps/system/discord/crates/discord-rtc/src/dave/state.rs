use std::borrow::Cow;
use std::collections::HashMap;
use std::num::NonZeroU16;

use davey::{Codec, DaveSession, MediaType, ProposalsOperationType};
use discord_core::{Privacy, VideoCodec};

use crate::dave::code::displayable;
use crate::dave::messages::{Input, Reply};
use crate::error::{Error, Result};
use crate::format::SILENCE;

pub(crate) const MAX_VERSION: u16 = davey::DAVE_PROTOCOL_VERSION;

const TRANSITION_EXPIRY: u32 = 10;

const DOWNGRADE_EXPIRY: u32 = 24;

const FAILURE_TOLERANCE: u32 = 36;

const MAGIC_MARKER: [u8; 2] = [0xFA, 0xFA];

#[derive(Debug)]
pub(crate) struct Dave {
    user: u64,
    channel: u64,
    version: u16,
    session: Option<DaveSession>,
    pending: HashMap<u16, u16>,
    last_transition: Option<u16>,
    downgraded: bool,
    reinitializing: bool,
    failures: u32,
}

impl Dave {
    pub(crate) fn new(
        user: u64,
        channel: u64,
        version: u16,
        replies: &mut Vec<Reply>,
    ) -> Result<Dave> {
        let mut dave = Dave {
            user,
            channel,
            version,
            session: None,
            pending: HashMap::new(),
            last_transition: None,
            downgraded: false,
            reinitializing: false,
            failures: 0,
        };
        dave.reinit(replies)?;
        Ok(dave)
    }

    pub(crate) fn privacy(&self) -> Privacy {
        if self.version == 0 {
            return Privacy::Transport;
        }
        let code = self
            .session
            .as_ref()
            .filter(|session| session.is_ready())
            .and_then(DaveSession::voice_privacy_code)
            .map(displayable);
        Privacy::EndToEnd { code }
    }

    pub(crate) fn handle(
        &mut self,
        input: Input,
        members: &[u64],
        replies: &mut Vec<Reply>,
    ) -> Result<()> {
        match input {
            Input::PrepareTransition {
                transition,
                version,
            } => self.prepare_transition(transition, version, replies),
            Input::ExecuteTransition { transition } => self.execute(transition),
            Input::PrepareEpoch { version, epoch } => {
                if epoch == 1 {
                    self.version = version;
                    return self.reinit(replies);
                }
            }
            Input::ExternalSender(sender) => {
                if let Some(session) = self.session.as_mut()
                    && let Err(error) = session.set_external_sender(&sender)
                {
                    tracing::warn!(%error, "DAVE external sender rejected");
                }
            }
            Input::Proposals { revoke, proposals } => {
                self.propose(revoke, &proposals, members, replies);
            }
            Input::Commit { transition, commit } => {
                let Some(session) = self.session.as_mut() else {
                    return Ok(());
                };
                let applied = session.process_commit(&commit).map_err(Error::dave);
                return self.transitioned(transition, applied, replies);
            }
            Input::Welcome {
                transition,
                welcome,
            } => {
                let Some(session) = self.session.as_mut() else {
                    return Ok(());
                };
                let joined = session.process_welcome(&welcome).map_err(Error::dave);
                return self.transitioned(transition, joined, replies);
            }
        }
        Ok(())
    }

    pub(crate) fn encrypt<'a>(&mut self, opus: &'a [u8]) -> Option<Cow<'a, [u8]>> {
        if opus == SILENCE {
            return Some(Cow::Borrowed(opus));
        }
        self.seal(MediaType::AUDIO, Codec::OPUS, opus)
    }

    pub(crate) fn encrypt_video<'a>(
        &mut self,
        codec: VideoCodec,
        frame: &'a [u8],
    ) -> Option<Cow<'a, [u8]>> {
        self.seal(MediaType::VIDEO, video_codec(codec), frame)
    }

    fn seal<'a>(
        &mut self,
        media: MediaType,
        codec: Codec,
        frame: &'a [u8],
    ) -> Option<Cow<'a, [u8]>> {
        match self.session.as_mut() {
            Some(session) if self.version != 0 && session.is_ready() => {
                match session.encrypt(media, codec, frame) {
                    Ok(sealed) => Some(sealed),
                    Err(error) => {
                        tracing::debug!(%error, "failed to encrypt DAVE frame");
                        None
                    }
                }
            }
            _ => Some(Cow::Borrowed(frame)),
        }
    }

    pub(crate) fn decrypt<'a>(
        &mut self,
        sender: Option<u64>,
        frame: &'a [u8],
        replies: &mut Vec<Reply>,
    ) -> Result<Option<Cow<'a, [u8]>>> {
        if frame == SILENCE {
            return Ok(Some(Cow::Borrowed(frame)));
        }
        self.open(MediaType::AUDIO, sender, frame, replies)
    }

    pub(crate) fn decrypt_video<'a>(
        &mut self,
        sender: Option<u64>,
        frame: &'a [u8],
        replies: &mut Vec<Reply>,
    ) -> Result<Option<Cow<'a, [u8]>>> {
        self.open(MediaType::VIDEO, sender, frame, replies)
    }

    fn open<'a>(
        &mut self,
        media: MediaType,
        sender: Option<u64>,
        frame: &'a [u8],
        replies: &mut Vec<Reply>,
    ) -> Result<Option<Cow<'a, [u8]>>> {
        let Some(session) = self.session.as_mut() else {
            return Ok(Some(Cow::Borrowed(frame)));
        };
        let decryptable = session.is_ready()
            && (self.version != 0 || sender.is_some_and(|user| session.can_passthrough(user)));
        if !decryptable {
            return Ok((!frame.ends_with(&MAGIC_MARKER)).then_some(Cow::Borrowed(frame)));
        }
        let Some(sender) = sender else {
            self.failed(replies)?;
            return Ok(None);
        };
        match session.decrypt(sender, media, frame) {
            Ok(plain) => {
                self.failures = 0;
                Ok(Some(Cow::Owned(plain)))
            }
            Err(error) => {
                tracing::trace!(%error, sender, "failed to decrypt DAVE frame");
                self.failed(replies)?;
                Ok(None)
            }
        }
    }

    fn reinit(&mut self, replies: &mut Vec<Reply>) -> Result<()> {
        let Some(version) = NonZeroU16::new(self.version) else {
            if let Some(session) = self.session.as_mut() {
                session.reset().map_err(Error::dave)?;
                session.set_passthrough_mode(true, Some(TRANSITION_EXPIRY));
            }
            return Ok(());
        };
        let session = match self.session.as_mut() {
            Some(session) => {
                session
                    .reinit(version, self.user, self.channel, None)
                    .map_err(Error::dave)?;
                session
            }
            None => self.session.insert(
                DaveSession::new(version, self.user, self.channel, None).map_err(Error::dave)?,
            ),
        };
        let package = session.create_key_package().map_err(Error::dave)?;
        replies.push(Reply::KeyPackage(package));
        Ok(())
    }

    fn prepare_transition(&mut self, transition: u16, version: u16, replies: &mut Vec<Reply>) {
        self.pending.insert(transition, version);
        if transition == 0 {
            self.execute(0);
            return;
        }
        if version == 0
            && let Some(session) = self.session.as_mut()
        {
            session.set_passthrough_mode(true, Some(DOWNGRADE_EXPIRY));
        }
        replies.push(Reply::TransitionReady(transition));
    }

    fn execute(&mut self, transition: u16) {
        let Some(version) = self.pending.remove(&transition) else {
            tracing::debug!(transition, "executed unprepared DAVE transition");
            return;
        };
        let previous = std::mem::replace(&mut self.version, version);
        if previous != version && version == 0 {
            self.downgraded = true;
        } else if transition > 0 && self.downgraded {
            self.downgraded = false;
            if let Some(session) = self.session.as_mut() {
                session.set_passthrough_mode(true, Some(TRANSITION_EXPIRY));
            }
        }
        self.reinitializing = false;
        self.last_transition = Some(transition);
    }

    fn propose(
        &mut self,
        revoke: bool,
        proposals: &[u8],
        members: &[u64],
        replies: &mut Vec<Reply>,
    ) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let operation = if revoke {
            ProposalsOperationType::REVOKE
        } else {
            ProposalsOperationType::APPEND
        };
        match session.process_proposals(operation, proposals, Some(members)) {
            Ok(Some(answer)) => replies.push(Reply::CommitWelcome {
                commit: answer.commit,
                welcome: answer.welcome,
            }),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "failed to process DAVE proposals"),
        }
    }

    fn transitioned(
        &mut self,
        transition: u16,
        outcome: Result<()>,
        replies: &mut Vec<Reply>,
    ) -> Result<()> {
        if let Err(error) = outcome {
            tracing::warn!(%error, transition, "failed to join DAVE group");
            return self.recover(transition, replies);
        }
        if transition == 0 {
            self.reinitializing = false;
            self.last_transition = Some(0);
        } else {
            self.pending.insert(transition, self.version);
            replies.push(Reply::TransitionReady(transition));
        }
        Ok(())
    }

    fn recover(&mut self, transition: u16, replies: &mut Vec<Reply>) -> Result<()> {
        if self.reinitializing {
            return Ok(());
        }
        self.reinitializing = true;
        self.failures = 0;
        replies.push(Reply::InvalidCommitWelcome(transition));
        self.reinit(replies)
    }

    fn failed(&mut self, replies: &mut Vec<Reply>) -> Result<()> {
        if self.reinitializing || !self.pending.is_empty() {
            return Ok(());
        }
        self.failures += 1;
        if self.failures <= FAILURE_TOLERANCE {
            return Ok(());
        }
        self.failures = 0;
        match self.last_transition {
            Some(transition) => self.recover(transition, replies),
            None => Ok(()),
        }
    }
}

fn video_codec(codec: VideoCodec) -> Codec {
    match codec {
        VideoCodec::Av1 => Codec::AV1,
        VideoCodec::H265 => Codec::H265,
        VideoCodec::H264 => Codec::H264,
        VideoCodec::Vp8 => Codec::VP8,
        VideoCodec::Vp9 => Codec::VP9,
    }
}

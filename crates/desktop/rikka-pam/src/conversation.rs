use std::ffi::{CStr, c_char, c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

use zeroize::Zeroize;

use crate::ffi::{
    BUF_ERR, CONV_ERR, ERROR_MSG, PROMPT_ECHO_OFF, PROMPT_ECHO_ON, PamMessage, PamResponse,
    SUCCESS, TEXT_INFO,
};
use crate::secret::Secret;
use crate::step::{Message, MessageKind, Prompt, Step};

pub(crate) struct Answer {
    pub(crate) serial: u64,
    pub(crate) secret: Secret,
}

pub(crate) struct Conversation<'a> {
    pub(crate) report: &'a mut dyn FnMut(Step),
    pub(crate) answers: &'a Receiver<Answer>,
    pub(crate) cancelled: &'a AtomicBool,
    pub(crate) serial: u64,
}

impl Conversation<'_> {
    fn ask(&mut self, text: String, echo: bool) -> Option<Secret> {
        if self.cancelled.load(Ordering::Acquire) {
            return None;
        }
        self.serial += 1;
        let serial = self.serial;
        (self.report)(Step::Prompt(Prompt { serial, text, echo }));
        loop {
            match self.answers.recv() {
                Ok(answer) if answer.serial == serial => return Some(answer.secret),
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }

    fn report_message(&mut self, kind: MessageKind, text: String) {
        (self.report)(Step::Message(Message { kind, text }));
    }
}

pub(crate) unsafe extern "C" fn converse(
    count: c_int,
    messages: *mut *const PamMessage,
    responses: *mut *mut PamResponse,
    conversation: *mut c_void,
) -> c_int {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        answer_all(
            count,
            messages,
            responses,
            conversation.cast::<Conversation<'_>>(),
        )
    }))
    .unwrap_or(CONV_ERR)
}

unsafe fn answer_all(
    count: c_int,
    messages: *mut *const PamMessage,
    responses: *mut *mut PamResponse,
    conversation: *mut Conversation<'_>,
) -> c_int {
    let Ok(count) = usize::try_from(count) else {
        return CONV_ERR;
    };
    if count == 0 || messages.is_null() || responses.is_null() || conversation.is_null() {
        return CONV_ERR;
    }
    let conversation = unsafe { &mut *conversation };
    let replies = unsafe { libc::calloc(count, size_of::<PamResponse>()) }.cast::<PamResponse>();
    if replies.is_null() {
        return BUF_ERR;
    }
    for index in 0..count {
        let message = unsafe { *messages.add(index) };
        if message.is_null() {
            unsafe { forget(replies, count) };
            return CONV_ERR;
        }
        let (style, text) = unsafe { ((*message).msg_style, text_of((*message).msg)) };
        let answered = match style {
            PROMPT_ECHO_OFF | PROMPT_ECHO_ON => {
                match conversation.ask(text, style == PROMPT_ECHO_ON) {
                    Some(secret) => match unsafe { owned_by_pam(&secret) } {
                        Some(copy) => {
                            unsafe { (*replies.add(index)).resp = copy };
                            SUCCESS
                        }
                        None => BUF_ERR,
                    },
                    None => CONV_ERR,
                }
            }
            ERROR_MSG => {
                conversation.report_message(MessageKind::Error, text);
                SUCCESS
            }
            TEXT_INFO => {
                conversation.report_message(MessageKind::Info, text);
                SUCCESS
            }
            _ => CONV_ERR,
        };
        if answered != SUCCESS {
            unsafe { forget(replies, count) };
            return answered;
        }
    }
    unsafe { *responses = replies };
    SUCCESS
}

unsafe fn text_of(text: *const c_char) -> String {
    match text.is_null() {
        true => String::new(),
        false => unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned(),
    }
}

unsafe fn owned_by_pam(secret: &Secret) -> Option<*mut c_char> {
    let bytes = secret.reveal().as_bytes();
    if bytes.contains(&0) {
        return None;
    }
    let copy = unsafe { libc::malloc(bytes.len() + 1) }.cast::<u8>();
    if copy.is_null() {
        return None;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), copy, bytes.len());
        *copy.add(bytes.len()) = 0;
    }
    Some(copy.cast())
}

unsafe fn forget(replies: *mut PamResponse, count: usize) {
    for index in 0..count {
        let reply = unsafe { &mut *replies.add(index) };
        if reply.resp.is_null() {
            continue;
        }
        let length = unsafe { libc::strlen(reply.resp) };
        unsafe { std::slice::from_raw_parts_mut(reply.resp.cast::<u8>(), length) }.zeroize();
        unsafe { libc::free(reply.resp.cast()) };
    }
    unsafe { libc::free(replies.cast()) };
}

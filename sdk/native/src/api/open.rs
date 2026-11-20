use crate::abi::violation;
use crate::error;
use crate::ffi;

fn is_absolute_url(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    let mut letters = scheme.chars();
    letters
        .next()
        .is_some_and(|letter| letter.is_ascii_alphabetic())
        && letters.all(|letter| letter.is_ascii_alphanumeric() || "+-.".contains(letter))
        && !rest.is_empty()
        && !url
            .chars()
            .any(|letter| letter.is_whitespace() || letter.is_control())
}

fn launch(target: &str) -> ffi::Status {
    match rikka_open::open(target) {
        Ok(()) => ffi::Status::Ok,
        Err(failure) => error::fail(ffi::Status::Failed, failure),
    }
}

pub(crate) fn open_url(url: &str) -> ffi::Status {
    if !is_absolute_url(url) {
        return error::fail(
            ffi::Status::Failed,
            format!("`{url}` is not an absolute URL"),
        );
    }
    launch(url)
}

pub(crate) fn open_file(path: &str) -> ffi::Status {
    if path.is_empty() {
        violation("the path is empty");
    }
    let absolute = match std::path::absolute(path) {
        Ok(absolute) => absolute,
        Err(failure) => {
            return error::fail(
                ffi::Status::Failed,
                format!("failed to resolve {path}: {failure}"),
            );
        }
    };
    if let Err(failure) = std::fs::metadata(&absolute) {
        return error::fail(
            ffi::Status::Failed,
            format!("failed to open {}: {failure}", absolute.display()),
        );
    }
    launch(&rikka_open::path_to_uri(&absolute))
}

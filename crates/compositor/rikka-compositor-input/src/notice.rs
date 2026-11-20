use crate::error::Error;
use crate::report::ConfigReport;

#[derive(Debug)]
pub enum InputNotice {
    Configured(ConfigReport),
    Failed(Error),
}

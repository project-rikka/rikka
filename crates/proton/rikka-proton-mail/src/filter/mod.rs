mod draft;
mod issue;
mod reading;
mod requests;
mod saved;
mod sieve;
mod simple;
mod writing;

pub use draft::FilterDraft;
pub use issue::{SieveIssue, SievePosition};
pub use saved::Filter;
pub use sieve::{SieveArgument, SieveCommand, SieveScript, SieveSyntaxError, SieveTest};
pub use simple::{Comparator, Condition, ConditionKind, FilterActions, Operator, SimpleFilter};

const FILTERS: &str = "mail/v4/filters";

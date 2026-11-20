use std::ops::RangeInclusive;

use crate::value::Value;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Constraint {
    #[default]
    Any,
    Range {
        min: Value,
        max: Value,
    },
    Choices(Vec<Value>),
}

impl Constraint {
    #[must_use]
    pub fn is_any(&self) -> bool {
        matches!(self, Self::Any)
    }

    pub fn check(&self, value: &Value) -> Result<(), String> {
        match self {
            Self::Any => Ok(()),
            Self::Range { min, max } => {
                let within = matches!(
                    (as_number(value), as_number(min), as_number(max)),
                    (Some(number), Some(low), Some(high)) if (low..=high).contains(&number)
                );
                if within {
                    Ok(())
                } else {
                    Err(format!("expected a number between {min} and {max}"))
                }
            }
            Self::Choices(allowed) => {
                if allowed.contains(value) {
                    Ok(())
                } else {
                    let names: Vec<String> = allowed
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect();
                    Err(format!("expected one of {}", names.join(", ")))
                }
            }
        }
    }
}

fn as_number(value: &Value) -> Option<f64> {
    match value {
        Value::Int(value) => Some(*value as f64),
        Value::Float(value) => Some(*value),
        _ => None,
    }
}

impl From<RangeInclusive<i64>> for Constraint {
    fn from(range: RangeInclusive<i64>) -> Self {
        Self::Range {
            min: Value::Int(*range.start()),
            max: Value::Int(*range.end()),
        }
    }
}

impl From<RangeInclusive<f64>> for Constraint {
    fn from(range: RangeInclusive<f64>) -> Self {
        Self::Range {
            min: Value::Float(*range.start()),
            max: Value::Float(*range.end()),
        }
    }
}

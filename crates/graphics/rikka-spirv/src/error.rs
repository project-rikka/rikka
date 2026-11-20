pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Length(usize),
    Header,
    Truncated { at: usize },
    MissingOperand,
    MissingEntryPoint(String),
    UndeclaredType(u32),
    NonPointerVariable(u32),
    NonConstantLength(u32),
    UnsizedPushMember(u32),
    UnlocatedInterface,
    NonNumericInterface,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Length(bytes) => write!(f, "{bytes} bytes are not whole 32-bit words"),
            Self::Header => f.write_str("missing module header"),
            Self::Truncated { at } => {
                write!(
                    f,
                    "instruction at word {at} runs past the end of the module"
                )
            }
            Self::MissingOperand => f.write_str("instruction is missing an operand"),
            Self::MissingEntryPoint(entry) => write!(f, "module has no entry point `{entry}`"),
            Self::UndeclaredType(id) => write!(f, "type %{id} is not declared"),
            Self::NonPointerVariable(id) => write!(f, "variable %{id} is not a pointer"),
            Self::NonConstantLength(id) => {
                write!(f, "array type %{id} has no constant length")
            }
            Self::UnsizedPushMember(id) => {
                write!(f, "push constant block member of type %{id} has no size")
            }
            Self::UnlocatedInterface => f.write_str("stage interface variable has no location"),
            Self::NonNumericInterface => f.write_str("stage interface variable is not numeric"),
        }
    }
}

impl std::error::Error for Error {}

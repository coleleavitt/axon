use std::borrow::Cow;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId(Cow<'static, str>);

impl ModuleId {
    pub fn new(value: impl Into<Cow<'static, str>>) -> Result<Self, ModuleIdError> {
        Ok(Self(validate_id("module", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ModuleId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<&'static str> for ModuleId {
    type Error = ModuleIdError;

    fn try_from(value: &'static str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for ModuleId {
    type Error = ModuleIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InputId(Cow<'static, str>);

impl InputId {
    pub fn new(value: impl Into<Cow<'static, str>>) -> Result<Self, InputIdError> {
        Ok(Self(validate_id("input", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for InputId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for InputId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl TryFrom<&'static str> for InputId {
    type Error = InputIdError;

    fn try_from(value: &'static str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<String> for InputId {
    type Error = InputIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EndpointId {
    Input(InputId),
    Module(ModuleId),
}

impl fmt::Display for EndpointId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(id) => write!(formatter, "input:{id}"),
            Self::Module(id) => write!(formatter, "module:{id}"),
        }
    }
}

impl From<InputId> for EndpointId {
    fn from(value: InputId) -> Self {
        Self::Input(value)
    }
}

impl From<ModuleId> for EndpointId {
    fn from(value: ModuleId) -> Self {
        Self::Module(value)
    }
}

fn validate_id(kind: &'static str, value: Cow<'static, str>) -> Result<Cow<'static, str>, IdError> {
    if value.is_empty() {
        return Err(IdError::Empty { kind });
    }
    if value.chars().any(char::is_whitespace) {
        return Err(IdError::ContainsWhitespace {
            kind,
            value: value.into_owned(),
        });
    }
    Ok(value)
}

pub type ModuleIdError = IdError;
pub type InputIdError = IdError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    Empty { kind: &'static str },
    ContainsWhitespace { kind: &'static str, value: String },
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { kind } => write!(formatter, "{kind} id cannot be empty"),
            Self::ContainsWhitespace { kind, value } => {
                write!(formatter, "{kind} id cannot contain whitespace: {value:?}")
            }
        }
    }
}

impl Error for IdError {}

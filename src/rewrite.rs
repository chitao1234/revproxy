use std::borrow::Cow;

use fancy_regex::Regex;
use serde::{
    de::{self},
    Deserialize, Deserializer, Serialize, Serializer,
};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct RewriteRules {
    /// Rege
    pub find: RegexWrapper,
    pub replace: String,
}

#[derive(Debug, Clone)]
pub struct RegexWrapper(Regex);

impl From<Regex> for RegexWrapper {
    fn from(value: Regex) -> Self {
        RegexWrapper(value)
    }
}

impl Into<Regex> for RegexWrapper {
    fn into(self) -> Regex {
        self.0
    }
}

impl PartialEq for RegexWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Serialize for RegexWrapper {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_newtype_struct("RegexWrapper", &self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for RegexWrapper {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = Cow::<'_, str>::deserialize(deserializer)?;
        Regex::new(&s).map(RegexWrapper).map_err(de::Error::custom)
    }
}

// #[derive(Debug)]
// pub enum RegexSerdeError {
//     Message(String),
// }

// impl ser::Error for RegexSerdeError {
//     fn custom<T: Display>(msg: T) -> Self {
//         RegexSerdeError::Message(msg.to_string())
//     }
// }

// impl de::Error for RegexSerdeError {
//     fn custom<T: Display>(msg: T) -> Self {
//         RegexSerdeError::Message(msg.to_string())
//     }
// }

// impl Display for RegexSerdeError {
//     fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             RegexSerdeError::Message(msg) => formatter.write_str(msg)
//         }
//     }
// }

// impl std::error::Error for RegexSerdeError {}

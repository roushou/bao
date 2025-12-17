use std::collections::HashMap;

use serde::{
    Deserialize,
    de::{self, Deserializer, MapAccess, SeqAccess, Visitor},
};
use toml::Spanned;

use super::{Arg, ArgType, Flag, default_true};

/// Arg with name field for array format deserialization
#[derive(Debug, Deserialize)]
pub(super) struct ArgWithName {
    name: String,
    #[serde(rename = "type")]
    arg_type: ArgType,
    #[serde(default = "default_true")]
    required: bool,
    description: Option<String>,
    default: Option<toml::Value>,
    #[serde(default)]
    choices: Option<Vec<String>>,
}

/// Flag with name field for array format deserialization
#[derive(Debug, Deserialize)]
pub(super) struct FlagWithName {
    name: String,
    #[serde(rename = "type", default)]
    flag_type: ArgType,
    short: Option<char>,
    description: Option<String>,
    default: Option<toml::Value>,
    #[serde(default)]
    choices: Option<Vec<String>>,
}

/// Untagged enum to support both array and map formats for args
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ArgsFormat {
    Array(Vec<ArgWithName>),
    Map(HashMap<String, Arg>),
}

impl From<ArgsFormat> for HashMap<String, Arg> {
    fn from(format: ArgsFormat) -> Self {
        match format {
            ArgsFormat::Array(vec) => vec
                .into_iter()
                .map(|a| {
                    (
                        a.name.clone(),
                        Arg {
                            arg_type: a.arg_type,
                            required: a.required,
                            description: a.description,
                            default: a.default,
                            choices: a.choices,
                        },
                    )
                })
                .collect(),
            ArgsFormat::Map(map) => map,
        }
    }
}

pub(super) fn deserialize_args<'de, D>(
    deserializer: D,
) -> std::result::Result<HashMap<String, Arg>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    ArgsFormat::deserialize(deserializer).map(Into::into)
}

/// Deserialize flags from either array or map format
/// Uses manual Visitor because Flag.short uses Spanned which doesn't work with untagged enums
pub(super) fn deserialize_flags<'de, D>(
    deserializer: D,
) -> std::result::Result<HashMap<String, Flag>, D::Error>
where
    D: Deserializer<'de>,
{
    struct FlagsVisitor;

    impl<'de> Visitor<'de> for FlagsVisitor {
        type Value = HashMap<String, Flag>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a map of flags or an array of flags with name field")
        }

        fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut map = HashMap::new();
            while let Some(item) = seq.next_element::<FlagWithName>()? {
                map.insert(
                    item.name.clone(),
                    Flag {
                        flag_type: item.flag_type,
                        // Use empty span for array format (span info not available)
                        short: item.short.map(|c| Spanned::new(0..0, c)),
                        description: item.description,
                        default: item.default,
                        choices: item.choices,
                    },
                );
            }
            Ok(map)
        }

        fn visit_map<M>(self, map: M) -> std::result::Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            HashMap::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(FlagsVisitor)
}

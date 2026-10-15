// The Python protocols require JSON strings. Serde's default enum decoder also
// accepts externally tagged objects such as {"approved": null}; reject those
// at the type boundary rather than letting each transport remember to do it.
macro_rules! string_enum {
    ($visibility:vis enum $name:ident ($field:literal) {
        $($variant:ident => $value:literal),+ $(,)?
    }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[serde(try_from = "String", into = "String")]
        $visibility enum $name { $($variant),+ }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }
        }

        impl TryFrom<String> for $name {
            type Error = crate::Error;

            fn try_from(value: String) -> crate::Result<Self> {
                match value.as_str() {
                    $($value => Ok(Self::$variant)),+,
                    _ => Err(crate::Error::InvalidField {
                        field: $field,
                        expected: concat!("a supported ", $field, " string"),
                    }),
                }
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self { value.as_str().to_owned() }
        }
    };
}

pub(crate) use string_enum;

use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use std::{collections::HashSet, fmt};

// Validate the original bytes before maps/Value can discard duplicate keys.
// Values are traversed without retaining a second copy of the record.
struct UniqueKeys;

impl<'de> Deserialize<'de> for UniqueKeys {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;

        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueKeys;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("JSON with unique object keys")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<UniqueKeys, M::Error> {
                let mut keys = HashSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !keys.insert(key.clone()) {
                        let excerpt: String = key.chars().take(80).collect();
                        return Err(de::Error::custom(format!(
                            "duplicate JSON object key {excerpt:?}"
                        )));
                    }
                    map.next_value::<UniqueKeys>()?;
                }
                Ok(UniqueKeys)
            }

            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<UniqueKeys, S::Error> {
                while seq.next_element::<UniqueKeys>()?.is_some() {}
                Ok(UniqueKeys)
            }

            fn visit_bool<E: de::Error>(self, _: bool) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_str<E: de::Error>(self, _: &str) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
            fn visit_unit<E: de::Error>(self) -> Result<UniqueKeys, E> {
                Ok(UniqueKeys)
            }
        }

        deserializer.deserialize_any(UniqueVisitor)
    }
}

pub(crate) fn validate_json(bytes: &[u8]) -> Result<(), serde_json::Error> {
    serde_json::from_slice::<UniqueKeys>(bytes).map(|_| ())
}

pub(crate) fn validate_json_reader(reader: impl std::io::Read) -> Result<(), serde_json::Error> {
    serde_json::from_reader::<_, UniqueKeys>(std::io::BufReader::new(reader)).map(|_| ())
}

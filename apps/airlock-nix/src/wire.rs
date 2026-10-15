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

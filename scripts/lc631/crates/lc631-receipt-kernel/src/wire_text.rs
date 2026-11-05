//! Size admission for receipt text, independent of authenticity and authority.
//!
//! Borrowing deserializers are checked before this module creates an owned
//! String. A deserializer may already own/allocate its input (including escaped
//! JSON); this module does not bound that upstream transport/parser allocation.

use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

pub(crate) fn bounded<'de, D, const LIMIT: usize>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct TextVisitor<const LIMIT: usize>;

    impl<'de, const LIMIT: usize> Visitor<'de> for TextVisitor<LIMIT> {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "receipt text of at most {LIMIT} UTF-8 bytes")
        }

        fn visit_str<E: Error>(self, value: &str) -> Result<String, E> {
            if value.len() > LIMIT {
                return Err(E::custom("receipt text size limit exceeded"));
            }
            Ok(value.to_owned())
        }

        fn visit_string<E: Error>(self, value: String) -> Result<String, E> {
            if value.len() > LIMIT {
                return Err(E::custom("receipt text size limit exceeded"));
            }
            Ok(value)
        }
    }

    deserializer.deserialize_string(TextVisitor::<LIMIT>)
}

pub(crate) fn schema<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    bounded::<D, { crate::RECEIPT_SCHEMA.len() }>(deserializer)
}

pub(crate) fn nonce<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    bounded::<D, 32>(deserializer)
}

pub(crate) fn digest<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    bounded::<D, 71>(deserializer)
}

pub(crate) fn optional_digest<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    // A private deserialization-only wrapper keeps the public Option<String>
    // shape while applying the same bounded visitor to the Some case.
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct DigestText(#[serde(deserialize_with = "digest")] String);

    Option::<DigestText>::deserialize(deserializer).map(|value| value.map(|value| value.0))
}

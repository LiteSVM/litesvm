use {
    serde::{de, Deserializer, Serializer},
    serde::{Deserialize, Serialize},
    std::str::FromStr,
};

pub fn serialize<T, S>(t: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: ToString,
    S: Serializer,
{
    t.to_string().serialize(serializer)
}

pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr,
    D: Deserializer<'de>,
    <T as FromStr>::Err: std::fmt::Debug,
{
    let s: String = String::deserialize(deserializer)?;
    s.parse()
        .map_err(|e| de::Error::custom(format!("Parse error: {e:?}")))
}

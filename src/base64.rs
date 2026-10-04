pub mod bytes{
    use serde::{Deserialize, Serializer, Deserializer};
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    pub fn serialize<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = STANDARD.encode(data);
        serializer.serialize_str(&encoded)
    }
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded: String = String::deserialize(deserializer)?;
        STANDARD.decode(encoded).map_err(serde::de::Error::custom)
    }
    pub fn deserialize_str<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded: String = String::deserialize(deserializer)?;
        Ok(encoded)
    }
}

pub mod string {
    use serde::{Deserialize, Serializer, Deserializer};
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    pub fn serialize<S>(data: &str, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = STANDARD.encode(data.as_bytes());
        serializer.serialize_str(&encoded)
    }
    pub fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded: String = String::deserialize(deserializer)?;
        let decoded = STANDARD.decode(encoded).map_err(serde::de::Error::custom)?;
        String::from_utf8(decoded).map_err(serde::de::Error::custom)
    }
}

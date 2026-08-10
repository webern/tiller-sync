use crate::error::Res;
use anyhow::{anyhow, Context};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use tokio::fs::ReadDir;

/// Write a file.
pub(crate) async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Res<()> {
    let path = path.as_ref();
    tokio::fs::write(path, contents)
        .await
        .context(format!("Unable to write to {}", path.to_string_lossy()))
}

/// Read a file to a `String`.
pub(crate) async fn read(path: &Path) -> Res<String> {
    tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read file at {}", path.display()))
}

/// Deserialize a JSON file into type `T`.
pub(crate) async fn deserialize<T>(path: &Path) -> Res<T>
where
    T: DeserializeOwned,
{
    let content = read(path).await?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON file at {}", path.display()))
}

pub(crate) async fn canonicalize(path: impl AsRef<Path>) -> Res<PathBuf> {
    tokio::fs::canonicalize(path.as_ref())
        .await
        .with_context(|| {
            format!(
                "Unable to canonicalize path '{}'",
                path.as_ref().to_string_lossy()
            )
        })
}

pub(crate) async fn make_dir(path: impl AsRef<Path>) -> Res<()> {
    tokio::fs::create_dir_all(path.as_ref())
        .await
        .with_context(|| {
            format!(
                "Unable to create directory at {}",
                path.as_ref().to_string_lossy()
            )
        })
}

pub(crate) async fn read_dir(path: impl AsRef<Path>) -> Res<ReadDir> {
    tokio::fs::read_dir(path.as_ref()).await.with_context(|| {
        format!(
            "Unable to run read_dir on {}",
            path.as_ref().to_string_lossy()
        )
    })
}

/// Copy a file from `from` to `to`.
pub(crate) async fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Res<u64> {
    tokio::fs::copy(from.as_ref(), to.as_ref())
        .await
        .with_context(|| {
            format!(
                "Unable to copy file from '{}' to '{}'",
                from.as_ref().to_string_lossy(),
                to.as_ref().to_string_lossy()
            )
        })
}

/// Remove a file.
pub(crate) async fn remove(path: impl AsRef<Path>) -> Res<()> {
    tokio::fs::remove_file(path.as_ref())
        .await
        .with_context(|| {
            format!(
                "Unable to remove file at '{}'",
                path.as_ref().to_string_lossy()
            )
        })
}

/// Parses update strings in "FIELD=VALUE" format into `("FIELD", "VALUE")`.
pub(crate) fn parse_key_val(key_val: &str) -> Res<(String, String)> {
    key_val
        .split_once('=')
        .map(|x| (x.0.to_string(), x.1.to_string()))
        .ok_or_else(|| anyhow!("Invalid format '{}', expected FIELD=VALUE", key_val))
}

/// Serializes and deserializes an `other_fields` argument as a JSON object.
///
/// Custom sheet columns are conceptually a map, and that is how MCP clients send them:
/// `{"other_fields": {"My Column": "value"}}`. On the command line, however, they arrive one
/// `--other-field Name=Value` occurrence at a time, and clap's derive only accumulates repeated
/// occurrences into a field whose type is literally spelled `Vec<_>`. Declaring the field as a map
/// made clap treat it as a single value of type `BTreeMap<String, String>` while the value parser
/// produced a `(String, String)`, which made every CLI invocation either fail as a missing required
/// argument or panic on the type mismatch.
///
/// So the field is a `Vec<(String, String)>` for clap's sake, and this module keeps the serialized
/// form a JSON object for everyone else. Each field also carries
/// `#[schemars(with = "BTreeMap<String, String>")]` so the MCP tool schema matches.
///
/// See <https://github.com/webern/tiller-sync/issues/41>.
pub(crate) mod other_fields {
    use serde::de::Deserializer;
    use serde::ser::Serializer;
    use serde::Deserialize;
    use std::collections::BTreeMap;

    pub(crate) fn serialize<S>(
        fields: &[(String, String)],
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let map: BTreeMap<&str, &str> = fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        serde::Serialize::serialize(&map, serializer)
    }

    pub(crate) fn deserialize<'de, D>(
        deserializer: D,
    ) -> std::result::Result<Vec<(String, String)>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = BTreeMap::<String, String>::deserialize(deserializer)?;
        Ok(map.into_iter().collect())
    }
}

/// Collects `other_fields` command-line pairs into the map form used by the model types.
///
/// When a key is repeated, the last occurrence wins.
pub(crate) fn other_fields_map(
    fields: impl IntoIterator<Item = (String, String)>,
) -> std::collections::BTreeMap<String, String> {
    fields.into_iter().collect()
}

/// Parses an amount string into an `Amount`.
pub(crate) fn parse_amount(s: &str) -> Res<crate::model::Amount> {
    s.parse()
        .with_context(|| format!("Invalid amount format: '{}'", s))
}

/// Generates a unique transaction ID for locally-created transactions.
///
/// The ID format is `user-` followed by a truncated UUIDv4 (dashes removed, truncated to 19
/// characters), resulting in IDs like `user-f47e8c2a9b3d4f1ea80`.
///
/// This distinguishes locally-created transactions from those created by Tiller, which use
/// 24-character hex IDs without a prefix.
pub fn generate_transaction_id() -> String {
    let uuid = uuid::Uuid::new_v4();
    let hex = uuid.as_simple().to_string(); // 32 hex chars, no dashes
    format!("user-{}", &hex[..19])
}

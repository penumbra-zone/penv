use std::fmt::{self, Display};

use anyhow::Result;
use camino::Utf8PathBuf;
use semver::Version;
use serde::{
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct as _,
    Deserialize, Deserializer, Serialize, Serializer,
};
use target_lexicon::Triple;

use super::{Asset, InstalledAsset, RawAsset};

/// Release information as deserialized from the GitHub API JSON,
/// prior to enriching.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RawRelease {
    tag_name: String,
    name: String,
    body: Option<String>,
    assets: Vec<RawAsset>,
}

/// Release information enriched with proper domain types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// The version of the release, parsed as semver.
    pub version: Version,
    /// The markdown formatted release notes.
    pub body: Option<String>,
    /// The collection of assets associated with the release, for all architectures.
    pub assets: Vec<Asset>,
    /// The name of the release on GitHub.
    pub name: String,
}

impl Display for Release {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.version)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledRelease {
    /// The version of the release, parsed as semver.
    pub version: Version,
    /// The markdown formatted release notes.
    pub body: Option<String>,
    /// The collection of assets installed as part of the release.
    pub assets: Vec<InstalledAsset>,
    /// The name of the release on GitHub.
    pub name: String,
}

impl Display for InstalledRelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.version)
    }
}

impl Serialize for InstalledRelease {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("InstalledRelease", 4)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("body", &self.body)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("assets", &self.assets)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for InstalledRelease {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Version,
            Body,
            Assets,
            Name,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`version`, `body`, `assets`, or `name`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "version" => Ok(Field::Version),
                            "body" => Ok(Field::Body),
                            "assets" => Ok(Field::Assets),
                            "name" => Ok(Field::Name),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct InstalledReleaseVisitor;

        impl<'de> Visitor<'de> for InstalledReleaseVisitor {
            type Value = InstalledRelease;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct InstalledRelease")
            }

            fn visit_map<V>(self, mut map: V) -> Result<InstalledRelease, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut version = None;
                let mut body = None;
                let mut assets = None;
                let mut name = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Version => {
                            if version.is_some() {
                                return Err(de::Error::duplicate_field("version"));
                            }
                            version = Some(map.next_value()?);
                        }
                        Field::Body => {
                            if body.is_some() {
                                return Err(de::Error::duplicate_field("body"));
                            }
                            body = Some(map.next_value()?);
                        }
                        Field::Assets => {
                            if assets.is_some() {
                                return Err(de::Error::duplicate_field("assets"));
                            }
                            assets = Some(map.next_value()?);
                        }
                        Field::Name => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                    }
                }

                let version = version.ok_or_else(|| de::Error::missing_field("version"))?;
                let body = body.ok_or_else(|| de::Error::missing_field("body"))?;
                let assets = assets.ok_or_else(|| de::Error::missing_field("assets"))?;
                let name = name.ok_or_else(|| de::Error::missing_field("name"))?;

                Ok(InstalledRelease {
                    version,
                    body,
                    assets,
                    name,
                })
            }
        }

        const FIELDS: &'static [&'static str] = &["version", "body", "assets", "name"];
        deserializer.deserialize_struct("InstalledRelease", FIELDS, InstalledReleaseVisitor)
    }
}

impl TryInto<Release> for RawRelease {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Release> {
        let version = Version::parse(&self.tag_name[1..])?;
        Ok(Release {
            version,
            body: self.body,
            assets: self
                .assets
                .into_iter()
                .map(|a| a.try_into())
                .collect::<Result<_>>()?,
            name: self.name,
        })
    }
}

impl TryInto<Release> for &RawRelease {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Release> {
        let version = Version::parse(&self.tag_name[1..])?;
        Ok(Release {
            version,
            body: self.body.clone(),
            assets: self
                .assets
                .clone()
                .into_iter()
                .map(|a| a.try_into())
                .collect::<Result<_>>()?,
            name: self.name.clone(),
        })
    }
}

impl PartialOrd for Release {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.version.partial_cmp(&other.version)
    }
}

impl Ord for Release {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.version.cmp(&other.version)
    }
}

/// Consists of the individual installable assets from a given release for the
/// desired architecture.
#[derive(Debug)]
pub(crate) struct InstallableRelease {
    pub(crate) pcli: Option<Vec<Utf8PathBuf>>,
    pub(crate) pclientd: Option<Vec<Utf8PathBuf>>,
    pub(crate) pd: Option<Vec<Utf8PathBuf>>,
    pub(crate) target_arch: Triple,
    /// The underlying release information.
    pub(crate) release: Release,
}

impl InstallableRelease {
    pub fn version(&self) -> &Version {
        &self.release.version
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use semver::Version;

    use super::*;

    #[test]
    fn deserialize_release() {
        let release = InstalledRelease {
            version: Version::parse("1.0.0").unwrap(),
            body: Some("Release notes for version 1.0.0".to_string()),
            assets: vec![InstalledAsset {
                target_arch: Triple::from_str("x86_64-unknown-linux-gnu").unwrap(),
                local_filepath: Utf8PathBuf::from("/tmp/fake"),
            }],
            name: "Release 1.0.0".to_string(),
        };

        // Serialize to TOML string
        eprintln!("{}", toml::to_string(&release).unwrap());

        // Example TOML string for deserialization
        let toml_str = r#"
            version = "1.0.0"
            body = "Release notes for version 1.0.0"
            name = "Release 1.0.0"

            [[assets]]
            target_arch = "x86_64-unknown-linux-gnu"
            local_filepath = "/tmp/fake"
        "#;

        // Deserialize from TOML string
        toml::from_str::<InstalledRelease>(toml_str).unwrap();
    }
}

use anyhow::{anyhow, Context as _, Result};
use std::{
    fmt::{self, Display},
    fs,
};

use camino::Utf8PathBuf;
use semver::Version;
use serde::{
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct as _,
    Deserialize, Deserializer, Serialize, Serializer,
};

use crate::pvm::release::{InstalledAsset, InstalledRelease};

use super::{Installable, InstallableBinaryRelease, UsableRelease};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledBinaryRelease {
    /// The version of the release, parsed as semver.
    pub version: Version,
    /// The markdown formatted release notes.
    pub body: Option<String>,
    /// The collection of assets installed as part of the release.
    pub assets: Vec<InstalledAsset>,
    /// The name of the release on GitHub.
    pub name: String,
    /// The root directory of the environment.
    pub root_dir: Utf8PathBuf,
}

impl Display for InstalledBinaryRelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.version)
    }
}

impl Serialize for InstalledBinaryRelease {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("InstalledRelease", 4)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("body", &self.body)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("root_dir", &self.root_dir)?;
        state.serialize_field("assets", &self.assets)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for InstalledBinaryRelease {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Version,
            Body,
            Assets,
            Name,
            RootDir,
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
                        formatter.write_str("`version`, `body`, `assets`, `root_dir`, or `name`")
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
                            "root_dir" => Ok(Field::RootDir),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct InstalledBinaryReleaseVisitor;

        impl<'de> Visitor<'de> for InstalledBinaryReleaseVisitor {
            type Value = InstalledBinaryRelease;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct InstalledBinaryRelease")
            }

            fn visit_map<V>(self, mut map: V) -> Result<InstalledBinaryRelease, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut version = None;
                let mut body = None;
                let mut assets = None;
                let mut name = None;
                let mut root_dir = None;

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
                        Field::RootDir => {
                            if root_dir.is_some() {
                                return Err(de::Error::duplicate_field("root_dir"));
                            }
                            root_dir = Some(map.next_value()?);
                        }
                    }
                }

                let version = version.ok_or_else(|| de::Error::missing_field("version"))?;
                let body = body.ok_or_else(|| de::Error::missing_field("body"))?;
                let assets = assets.ok_or_else(|| de::Error::missing_field("assets"))?;
                let name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                let root_dir = root_dir.ok_or_else(|| de::Error::missing_field("root_dir"))?;

                Ok(InstalledBinaryRelease {
                    version,
                    body,
                    assets,
                    name,
                    root_dir,
                })
            }
        }

        const FIELDS: &'static [&'static str] = &["version", "body", "assets", "name", "root_dir"];
        deserializer.deserialize_struct(
            "InstalledBinaryRelease",
            FIELDS,
            InstalledBinaryReleaseVisitor,
        )
    }
}

impl Installable for InstallableBinaryRelease {
    fn version(&self) -> Option<&Version> {
        Some(&self.release.version)
    }

    fn install(&self, version_path: Utf8PathBuf) -> Result<InstalledRelease> {
        // TODO: reuse fs code
        let mut installed_assets = Vec::new();
        let version_bin_path = version_path.join("bin");
        let file = self.pcli.as_ref().expect("expected pcli file");
        let metadata = fs::metadata(file)?;

        if !metadata.is_file() {
            return Err(anyhow!("missing pcli"));
        }

        let file_path = version_bin_path.join(file.file_name().expect("expected file name"));

        tracing::debug!("copying: {} to {}", file, file_path);
        fs::copy(file, &file_path)?;

        installed_assets.push(InstalledAsset {
            target_arch: self.target_arch.clone(),
            local_filepath: file_path,
        });

        let file = self.pd.as_ref().expect("expected pd file");
        let metadata = fs::metadata(file)?;

        if !metadata.is_file() {
            return Err(anyhow!("missing pd"));
        }

        let file_path = version_bin_path.join(file.file_name().expect("expected file name"));

        tracing::debug!("copying: {} to {}", file, file_path);
        fs::copy(file, &file_path)?;

        installed_assets.push(InstalledAsset {
            target_arch: self.target_arch.clone(),
            local_filepath: file_path,
        });

        let file = self.pclientd.as_ref().expect("expected pclientd file");
        let metadata = fs::metadata(file)?;

        if !metadata.is_file() {
            return Err(anyhow!("missing pclientd"));
        }

        let file_path = version_bin_path.join(file.file_name().expect("expected file name"));

        tracing::debug!("copying: {} to {}", file, file_path);
        fs::copy(file, &file_path)?;

        installed_assets.push(InstalledAsset {
            target_arch: self.target_arch.clone(),
            local_filepath: file_path,
        });

        Ok(InstalledRelease::Binary(InstalledBinaryRelease {
            version: self.version().clone(),
            body: self.release.body.clone(),
            assets: installed_assets,
            name: self.release.name.clone(),
            root_dir: version_path,
        }))
    }
}

impl UsableRelease for InstalledBinaryRelease {
    fn assets(&self) -> &[InstalledAsset] {
        &self.assets
    }

    fn uninstall(self) -> Result<()> {
        let installed_version_dir = &self.root_dir;
        if installed_version_dir.exists() {
            tracing::debug!("deleting version directory: {}", installed_version_dir);
            std::fs::remove_dir_all(&installed_version_dir)
                .context("error removing version directory")?;
        }

        Ok(())
    }
}

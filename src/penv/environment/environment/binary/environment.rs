use anyhow::{Context as _, Result};
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct as _;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::fs;

use camino::Utf8PathBuf;
use semver::Version;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::penv::{
    cache::cache::Cache,
    environment::{
        create_symlink, Binary as _, EnvironmentMetadata, EnvironmentTrait, ManagedFile,
    },
    release::{RepoOrVersion, VersionReqOrLatest},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BinaryEnvironment {
    /// Fields common to all environment types.
    pub metadata: EnvironmentMetadata,
    /// The version_requirement is only set for binary releases.
    ///
    /// For git checkouts, there is no version -- the state of the checkout
    /// defines the code that will run.
    pub version_requirement: VersionReqOrLatest,
    // TODO: implement a way to update the pinned_version
    // to the latest matching the version_requirement
    /// The pinned_version is only set for binary releases.
    ///
    /// For git checkouts, there is no version -- the state of the checkout
    /// defines the code that will run.
    pub pinned_version: Version,
}

impl Serialize for BinaryEnvironment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("BinaryEnvironment", 3)?;
        state.serialize_field("pinned_version", &self.pinned_version)?;
        state.serialize_field("version_requirement", &self.version_requirement)?;
        state.serialize_field("metadata", &self.metadata)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for BinaryEnvironment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Metadata,
            PinnedVersion,
            VersionRequirement,
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
                        formatter
                            .write_str("`metadata`, `pinned_version`, or `version_requirement`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "metadata" => Ok(Field::Metadata),
                            "pinned_version" => Ok(Field::PinnedVersion),
                            "version_requirement" => Ok(Field::VersionRequirement),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct BinaryEnvironmentVisitor;

        impl<'de> Visitor<'de> for BinaryEnvironmentVisitor {
            type Value = BinaryEnvironment;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct BinaryEnvironment")
            }

            fn visit_map<V>(self, mut map: V) -> Result<BinaryEnvironment, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut metadata: Option<EnvironmentMetadata> = None;
                let mut pinned_version: Option<Version> = None;
                let mut version_requirement: Option<VersionReqOrLatest> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Metadata => {
                            if metadata.is_some() {
                                return Err(de::Error::duplicate_field("metadata"));
                            }
                            metadata = Some(map.next_value()?);
                        }
                        Field::PinnedVersion => {
                            if pinned_version.is_some() {
                                return Err(de::Error::duplicate_field("pinned_version"));
                            }
                            pinned_version = Some(map.next_value()?);
                        }
                        Field::VersionRequirement => {
                            if version_requirement.is_some() {
                                return Err(de::Error::duplicate_field("version_requirement"));
                            }
                            version_requirement = Some(map.next_value()?);
                        }
                    }
                }

                let metadata = metadata.ok_or_else(|| de::Error::missing_field("metadata"))?;
                let pinned_version =
                    pinned_version.ok_or_else(|| de::Error::missing_field("pinned_version"))?;
                let version_requirement = version_requirement
                    .ok_or_else(|| de::Error::missing_field("version_requirement"))?;

                Ok(BinaryEnvironment {
                    metadata,
                    version_requirement,
                    pinned_version,
                })
            }
        }

        const FIELDS: &'static [&'static str] =
            &["metadata", "version_requirement", "pinned_version"];
        deserializer.deserialize_struct("BinaryEnvironment", FIELDS, BinaryEnvironmentVisitor)
    }
}

impl ManagedFile for BinaryEnvironment {
    fn path(&self) -> Utf8PathBuf {
        self.metadata.root_dir.clone()
    }
}

impl Display for BinaryEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Alias: {}", self.metadata.alias)?;
        writeln!(f, "GRPC URL: {}", self.metadata.grpc_url)?;
        writeln!(f, "Version Requirement: {}", self.version_requirement)?;
        writeln!(f, "Pinned Version: {}", self.pinned_version)?;
        writeln!(f, "Root Directory: {}", self.metadata.root_dir)?;
        writeln!(f, "Include Node: {}", !self.metadata.client_only)?;
        writeln!(
            f,
            "Generated Dev Network: {}",
            self.metadata.generate_network
        )
    }
}

impl EnvironmentTrait for BinaryEnvironment {
    /// Initializes an environment on disk, by creating the necessary
    /// pd/pclientd/pcli configurations and symlinks to the
    /// pinned version of the software stack.
    fn initialize(&self, cache: &Cache) -> Result<()> {
        // Create the directory structure for the environment
        let bin_dir = self.path().join("bin");
        tracing::debug!("creating bin_dir at {}", bin_dir);
        fs::create_dir_all(&bin_dir)
            .with_context(|| format!("Failed to create bin directory {}", bin_dir))?;

        // Since the initialization is version-dependent, it is necessary
        // to shell out to the installed binary to perform the initialization.
        //
        // Create symlinks for the pinned version of the software stack
        self.create_symlinks(cache)?;

        // If the environment is set to generate a local dev network,
        // we must initialize that prior to pcli and pclientd.
        // Initialize pcli configuration
        let pcli_binary = self.get_pcli_binary();
        let seed_phrase = pcli_binary.initialize(None)?;
        // TODO: lol don't do this
        tracing::debug!("seed phrase: {}", seed_phrase);
        let pclientd_binary = self.get_pclientd_binary();
        pclientd_binary.initialize(Some(HashMap::from([(
            // pass the seed phrase here to avoid keeping in memory long-term
            "seed_phrase".to_string(),
            seed_phrase,
        )])))?;
        if !self.metadata().client_only {
            let pd_binary = self.get_pd_binary();
            let mut pd_configs = HashMap::from([
                (
                    "external-address".to_string(),
                    // TODO: make configurable
                    "0.0.0.0:26656".to_string(),
                ),
                ("moniker".to_string(), self.metadata().alias.to_string()),
            ]);

            if self.metadata().generate_network {
                pd_configs.insert("generate_network".to_string(), "true".to_string());
            }

            pd_binary.initialize(Some(pd_configs))?;
        }

        Ok(())
    }

    fn create_symlinks(&self, cache: &Cache) -> Result<()> {
        let pinned_version = &self.pinned_version;

        create_symlink(
            cache.get_pcli_for_version(&pinned_version).ok_or_else(|| {
                anyhow::anyhow!(
                    "No installed pcli version found for version {}",
                    pinned_version
                )
            })?,
            &self.pcli_path(),
        )
        .context("error creating pcli symlink")?;
        create_symlink(
            cache
                .get_pclientd_for_version(&pinned_version)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "No installed pclientd version found for version {}",
                        pinned_version
                    )
                })?,
            &self.pclientd_path(),
        )
        .context("error creating pclientd symlink")?;
        if !self.metadata().client_only {
            create_symlink(
                cache.get_pd_for_version(&pinned_version).ok_or_else(|| {
                    anyhow::anyhow!(
                        "No installed pd version found for version {}",
                        pinned_version
                    )
                })?,
                &self.pd_path(),
            )
            .context("error creating pd symlink")?;
        }

        Ok(())
    }

    fn remove_symlinks(&self) -> Result<()> {
        fs::remove_file(&self.pcli_path())?;
        fs::remove_file(&self.pclientd_path())?;
        if !self.metadata().client_only {
            fs::remove_file(&self.pd_path())?;
        }

        Ok(())
    }

    fn satisfied_by_version(&self, version: &RepoOrVersion) -> bool {
        match (&self.version_requirement, version) {
            (VersionReqOrLatest::VersionReq(version_req), RepoOrVersion::Version(version)) => {
                version_req.matches(version)
            }
            (VersionReqOrLatest::Latest, RepoOrVersion::Version(_version)) => {
                unimplemented!("don't have latest version here")
            }
            // Latest never satisfied by a checkout
            (VersionReqOrLatest::Latest, RepoOrVersion::Repo(_repo)) => false,
            // A checkout environment is never satisfied by a binary version
            (VersionReqOrLatest::VersionReq(_version_req), RepoOrVersion::Repo(_repo)) => false,
        }
    }

    fn metadata(&self) -> &EnvironmentMetadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {

    use semver::Version;

    use crate::penv::release::VersionReqOrLatest;

    use super::*;

    #[test]
    fn deserialize_binary_environment() {
        let metadata = EnvironmentMetadata {
            alias: "test".to_string(),
            grpc_url: "http://localhost:9090".try_into().expect("ok"),
            root_dir: "/tmp/fake".into(),
            client_only: false,
            generate_network: true,
            pd_join_url: "http://localhost:9090".try_into().expect("ok"),
        };

        // Serialize to TOML string
        eprintln!("{}", toml::to_string(&metadata).unwrap());

        let toml_str = r#"
            alias = "test"
            grpc_url = "http://localhost:9090/"
            root_dir = "/tmp/fake"
            pd_join_url = "http://localhost:9090/"
            client_only = false
            generate_network = true
        "#;

        toml::from_str::<EnvironmentMetadata>(toml_str).unwrap();

        let env = BinaryEnvironment {
            metadata: metadata.clone(),
            version_requirement: VersionReqOrLatest::Latest,
            pinned_version: Version::new(1, 0, 0),
        };

        // Serialize to TOML string
        eprintln!("{}", toml::to_string(&env).unwrap());

        // Example TOML string for deserialization
        let toml_str = r#"
            pinned_version = "1.0.0"

            [version_requirement]
            type = "Latest"

            [metadata]
            alias = "test"
            grpc_url = "http://localhost:9090/"
            root_dir = "/tmp/fake"
            pd_join_url = "http://localhost:9090/"
            client_only = false
            generate_network = true
        "#;

        // Deserialize from TOML string
        toml::from_str::<BinaryEnvironment>(toml_str).unwrap();
    }
}

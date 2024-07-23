use std::{
    fmt,
    fs::{self, File},
    io::Write as _,
    sync::Arc,
};

use anyhow::{anyhow, Context as _, Result};
use camino::Utf8PathBuf;
use semver::VersionReq;
use serde::{
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct as _,
    Deserialize, Deserializer, Serialize, Serializer,
};
use target_lexicon::Triple;
use url::Url;

use crate::pvm::release::Release;

use super::{
    cache::cache::Cache,
    downloader::Downloader,
    environment::{create_symlink, Environment, Environments},
    release::VersionOrLatest,
};

/// The top-level type for the Penumbra Version Manager.
///
/// This type encapsulates application state and exposes higher-level
/// operations.
pub struct Pvm {
    pub cache: Cache,
    pub(crate) downloader: Downloader,
    pub environments: Environments,
    pub repository_name: String,
    pub home_dir: Utf8PathBuf,
    pub active_environment: Option<Arc<Environment>>,
}

impl Serialize for Pvm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Pvm", 4)?;
        state.serialize_field("repository_name", &self.repository_name)?;
        state.serialize_field("home_dir", &self.home_dir)?;
        state.serialize_field(
            "active_environment",
            &self.active_environment.clone().map(|e| e.alias.clone()),
        )?;
        state.serialize_field("environments", &self.environments)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Pvm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Environments,
            RepositoryName,
            HomeDir,
            ActiveEnvironment,
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
                        formatter.write_str("`repository_name`, `home_dir`, `active_environment`, or `environments`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "repository_name" => Ok(Field::RepositoryName),
                            "home_dir" => Ok(Field::HomeDir),
                            "environments" => Ok(Field::Environments),
                            "active_environment" => Ok(Field::ActiveEnvironment),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct PvmVisitor;

        impl<'de> Visitor<'de> for PvmVisitor {
            type Value = Pvm;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Pvm")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Pvm, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut repository_name: Option<String> = None;
                let mut home_dir: Option<Utf8PathBuf> = None;
                let mut environments: Option<Environments> = None;
                let mut active_environment_alias: Option<String> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::RepositoryName => {
                            if repository_name.is_some() {
                                return Err(de::Error::duplicate_field("repository_name"));
                            }
                            repository_name = Some(map.next_value()?);
                        }
                        Field::HomeDir => {
                            if home_dir.is_some() {
                                return Err(de::Error::duplicate_field("home_dir"));
                            }
                            home_dir = Some(map.next_value()?);
                        }
                        Field::Environments => {
                            if environments.is_some() {
                                return Err(de::Error::duplicate_field("environments"));
                            }
                            environments = Some(map.next_value()?);
                        }
                        Field::ActiveEnvironment => {
                            if active_environment_alias.is_some() {
                                return Err(de::Error::duplicate_field("active_environment"));
                            }
                            active_environment_alias = Some(map.next_value()?);
                        }
                    }
                }

                let repository_name =
                    repository_name.ok_or_else(|| de::Error::missing_field("repository_name"))?;
                let home_dir = home_dir.ok_or_else(|| de::Error::missing_field("home_dir"))?;
                let environments =
                    environments.ok_or_else(|| de::Error::missing_field("environments"))?;
                let active_environment = active_environment_alias.and_then(|alias| {
                    environments
                        .iter()
                        .find(|e| e.alias == alias)
                        .map(|e| e.clone())
                });

                Ok(Pvm {
                    repository_name: repository_name.clone(),
                    home_dir: home_dir.clone(),
                    environments,
                    cache: Cache::new(home_dir.into()).map_err(de::Error::custom)?,
                    downloader: Downloader::new(repository_name).map_err(de::Error::custom)?,
                    active_environment,
                })
            }
        }

        const FIELDS: &'static [&'static str] = &[
            "repository_name",
            "home_dir",
            "environments",
            "active_environment",
        ];
        deserializer.deserialize_struct("Pvm", FIELDS, PvmVisitor)
    }
}

impl Pvm {
    /// Create a new instance of the Penumbra Version Manager.
    pub fn new(home: Utf8PathBuf) -> Result<Self> {
        // read config file to fetch existing environments
        let pvm_path = home.join("pvm.toml");
        let metadata = fs::metadata(&pvm_path);

        let pvm = if metadata.is_err() || !metadata.unwrap().is_file() {
            Self {
                cache: Cache::new(home.clone())?,
                downloader: Downloader::new("penumbra-zone/penumbra".to_string())?,
                environments: Environments {
                    environments: Vec::new(),
                },
                // TODO: shouldn't be hardcoded here
                repository_name: "penumbra-zone/penumbra".to_string(),
                home_dir: home,
                active_environment: None,
            }
        } else {
            let pvm_contents = fs::read_to_string(pvm_path)?;
            toml::from_str(&pvm_contents)?
        };

        tracing::debug!(environments=?pvm.environments, installed_releases=?pvm.cache.data.installed_releases, "created pvm with environments");
        Ok(pvm)
    }

    // TODO: delete this method and handle alternative repositories better
    pub fn new_from_repository(repository_name: String, home: Utf8PathBuf) -> Result<Self> {
        // read config file to fetch existing environments
        let pvm_path = home.join("pvm.toml");
        let metadata = fs::metadata(&pvm_path);

        let pvm = if metadata.is_err() || !metadata.unwrap().is_file() {
            Self {
                cache: Cache::new(home.clone())?,
                downloader: Downloader::new(repository_name.clone())?,
                environments: Environments {
                    environments: Vec::new(),
                },
                repository_name,
                home_dir: home.clone(),
                active_environment: None,
            }
        } else {
            let pvm_contents = fs::read_to_string(pvm_path)?;
            toml::from_str(&pvm_contents)?
        };

        tracing::debug!(environments=?pvm.environments, installed_releases=?pvm.cache.data.installed_releases, "created pvm with environments");
        Ok(pvm)
    }

    pub fn delete_environment(&mut self, environment_alias: String) -> Result<()> {
        if !self
            .environments
            .iter()
            .any(|e| e.alias == environment_alias)
        {
            return Err(anyhow!(
                "Environment with alias {} does not exist",
                environment_alias
            ));
        }

        // Get the matching environment
        // TODO: move this into an impl on Environments
        let environment = self
            .environments
            .iter()
            .find(|e| e.alias == environment_alias)
            .unwrap();

        if self.active_environment == Some(environment.clone()) {
            return Err(anyhow!(
                "refusing to delete active environment {}; perhaps you mean to `pvm deactivate` first",
                environment_alias
            ));
        }

        // Remove the environment from disk
        let env_path = &environment.root_dir;
        if env_path.exists() {
            tracing::debug!("removing environment directory: {}", env_path);
            std::fs::remove_dir_all(&env_path)?;
        }

        // Remove the environment from the app
        self.environments.retain(|e| e.alias != environment_alias);

        // Persist the updated state
        self.persist()?;

        println!("deleted environment {}", environment_alias);

        Ok(())
    }

    pub fn create_environment(
        &mut self,
        environment_alias: String,
        penumbra_version: VersionReq,
        grpc_url: Url,
        // eventually allow auto-download
        _repository_name: String,
        client_only: bool,
    ) -> Result<Arc<Environment>> {
        if self
            .environments
            .iter()
            .any(|e| e.alias == environment_alias)
        {
            return Err(anyhow!(
                "Environment with alias {} already exists",
                environment_alias
            ));
        }

        // Find the best matching version
        let cache = &self.cache;
        let matching_installed_version = match cache.find_best_match(&penumbra_version) {
            Some(installed_version) => installed_version,
            None => {
                // TODO: allow auto-installing here
                return Err(anyhow!(
                    "No installed version found for version requirement {}",
                    penumbra_version
                ));
            }
        };

        let root_dir = cache
            .home
            .join("environments")
            .join(environment_alias.clone());

        let environment = Arc::new(Environment {
            alias: environment_alias.clone(),
            version_requirement: penumbra_version.clone(),
            pinned_version: matching_installed_version.version.clone(),
            grpc_url: grpc_url.clone(),
            root_dir,
            client_only,
        });

        tracing::debug!("initializing environment");
        environment.initialize(&cache)?;

        tracing::debug!("created environment: {:?}", environment);

        // Add a reference to the environment to the app
        self.environments.push(environment.clone());

        self.persist()?;

        Ok(environment)
    }

    /// Returns all available versions and whether they're installed, optionally matching a given semver version requirement.
    pub async fn list_available(
        &self,
        required_version: Option<&semver::VersionReq>,
    ) -> Result<Vec<(Release, bool)>> {
        self.cache
            .list_available(required_version, &self.downloader)
            .await
    }

    pub async fn install_release(
        &mut self,
        penumbra_version: VersionOrLatest,
        target_arch: Triple,
    ) -> Result<()> {
        let downloader = &self.downloader;
        let releases = downloader.fetch_releases().await?;

        let mut candidate_releases = Vec::new();
        let latest_version = releases
            .iter()
            .max()
            .ok_or_else(|| anyhow!("No releases found"))?
            .version
            .clone();

        // 3b. find all the versions that satisfy the semver requirement
        'outer: for release in releases {
            if penumbra_version.matches(&release.version, &latest_version) {
                let release_name = release.name.clone();
                tracing::debug!("found candidate release {}", release_name);
                let enriched_release: Release = match release.try_into() {
                    Ok(enriched_release) => enriched_release,
                    Err(e) => {
                        tracing::debug!(
                            "failed to enrich release {}, not making an install candidate: {}",
                            release_name,
                            e
                        );
                        continue;
                    }
                };

                // Typically a release should contain all assets for all architectures,
                // but if it doesn't, this may produce unexpected failures.
                //
                // If the candidate release has no assets for the target architecture, skip it
                let has_arch_asset = enriched_release.assets.iter().any(|asset| {
                    asset.target_arch.is_some()
                        && asset.target_arch.clone().unwrap() == Triple::host()
                });
                if !has_arch_asset {
                    tracing::debug!(
                        "skipping release {} because it has no assets for the target architecture",
                        enriched_release.name
                    );
                    continue 'outer;
                }

                candidate_releases.push(enriched_release);
            }
        }

        if candidate_releases.is_empty() {
            return Err(anyhow!("No matching release found for version requirement"));
        }

        // 4. identify the latest candidate version
        let mut sorted_releases = candidate_releases.clone();
        sorted_releases.sort();

        let latest_release = sorted_releases.last().unwrap();

        // Skip installation if the installed_releases already contains this release
        let cache = &mut self.cache;
        if cache
            .data
            .installed_releases
            .iter()
            .any(|r| r.version == latest_release.version)
        {
            println!("release {} already installed", latest_release.version);
            return Ok(());
        }

        println!(
            "downloading latest matching release: {}",
            latest_release.version
        );
        let installable_release = downloader
            .download_release(latest_release, format!("{}", target_arch))
            .await?;
        tracing::debug!("installable release prepared: {:?}", installable_release);

        // 5. attempt to install to the cache
        println!(
            "installing latest matching release: {}",
            latest_release.version
        );
        cache.install_release(&installable_release)?;

        self.persist()?;

        Ok(())
    }

    pub fn pvm_file_path(&self) -> Utf8PathBuf {
        self.home_dir.join("pvm.toml")
    }

    pub fn persist(&self) -> Result<()> {
        fs::create_dir_all(&self.home_dir)
            .with_context(|| format!("Failed to create home directory {}", self.home_dir))?;

        let toml_pvm = toml::to_string(&self)?;

        tracing::debug!(pvm_file_path=?self.pvm_file_path(),"create file");

        let mut file = File::create(self.pvm_file_path())?;
        file.write_all(toml_pvm.as_bytes())?;

        tracing::debug!("persist cache");
        self.cache.persist()?;

        Ok(())
    }

    pub fn environment_info(&self, environment_alias: String) -> Result<&Environment> {
        let environment = self
            .environments
            .iter()
            .find(|e| e.alias == environment_alias)
            .ok_or_else(|| anyhow!("Environment with alias {} not found", environment_alias))?;

        Ok(environment)
    }

    pub fn environments(&self) -> Result<&Environments> {
        Ok(&self.environments)
    }

    pub fn activate(&mut self, environment_alias: String) -> Result<&Environment> {
        let environment = self
            .environments
            .iter()
            .find(|e| e.alias == environment_alias)
            .ok_or_else(|| anyhow!("Environment with alias {} not found", environment_alias))?;

        self.active_environment = Some(environment.clone());

        // Symlink the active environment's bin directory
        create_symlink(
            &environment.clone().root_dir.join("bin"),
            &self.home_dir.join("bin"),
        )
        .context("error creating pcli symlink")?;

        Ok(environment)
    }

    pub fn path_string(&self) -> String {
        self.home_dir.join("bin").to_string()
    }
}

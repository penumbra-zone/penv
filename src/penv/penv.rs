use std::{
    fmt,
    fs::{self, File},
    io::Write as _,
    process::Command,
    sync::Arc,
};

use anyhow::{anyhow, Context as _, Result};
use camino::Utf8PathBuf;
use semver::Version;
use serde::{
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct as _,
    Deserialize, Deserializer, Serialize, Serializer,
};
use sha2::Digest;
use sha2::Sha256;
use target_lexicon::Triple;
use url::Url;

use crate::penv::{
    cache::cache::CacheData,
    environment::{
        BinaryEnvironment, CheckoutEnvironment, Environment, EnvironmentMetadata, EnvironmentTrait,
        ManagedFile,
    },
    release::{
        git_repo::RepoMetadata, InstallableRelease, InstalledRelease, Release, RepoOrVersion,
    },
};

use super::{
    cache::cache::Cache,
    downloader::Downloader,
    environment::{create_symlink, Environments},
    release::RepoOrVersionReq,
};

/// The top-level type for the Penumbra Version Manager.
///
/// This type encapsulates application state and exposes higher-level
/// operations.
pub struct Penv {
    pub cache: Cache,
    pub(crate) downloader: Downloader,
    pub environments: Environments,
    pub repository_name: String,
    pub home_dir: Utf8PathBuf,
    pub active_environment: Option<Arc<Environment>>,
}

impl Serialize for Penv {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Penv", 5)?;
        state.serialize_field("repository_name", &self.repository_name)?;
        state.serialize_field("home_dir", &self.home_dir)?;
        state.serialize_field(
            "active_environment",
            &self
                .active_environment
                .clone()
                .map(|e| e.metadata().alias.clone()),
        )?;
        state.serialize_field("cache", &self.cache.data)?;
        state.serialize_field("environments", &self.environments)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Penv {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Environments,
            RepositoryName,
            HomeDir,
            ActiveEnvironment,
            Cache,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl Visitor<'_> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`repository_name`, `home_dir`, `active_environment`, `cache`, or `environments`")
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
                            "cache" => Ok(Field::Cache),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct PenvVisitor;

        impl<'de> Visitor<'de> for PenvVisitor {
            type Value = Penv;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Penv")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Penv, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut repository_name: Option<String> = None;
                let mut home_dir: Option<Utf8PathBuf> = None;
                let mut environments: Option<Environments> = None;
                let mut active_environment_alias: Option<String> = None;
                let mut cache: Option<CacheData> = None;

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
                        Field::Cache => {
                            if cache.is_some() {
                                return Err(de::Error::duplicate_field("cache"));
                            }
                            cache = Some(map.next_value()?);
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
                        .find(|e| e.metadata().alias == alias)
                        .cloned()
                });
                let cache = Cache {
                    home: home_dir.clone(),
                    data: cache.ok_or_else(|| de::Error::missing_field("cache"))?,
                };

                Ok(Penv {
                    repository_name: repository_name.clone(),
                    home_dir: home_dir.clone(),
                    environments,
                    cache,
                    downloader: Downloader::new(repository_name).map_err(de::Error::custom)?,
                    active_environment,
                })
            }
        }

        const FIELDS: &[&str] = &[
            "repository_name",
            "home_dir",
            "environments",
            "active_environment",
        ];
        deserializer.deserialize_struct("Penv", FIELDS, PenvVisitor)
    }
}

impl Penv {
    /// Create a new instance of the Penumbra Environment Manager.
    pub fn new(home: Utf8PathBuf) -> Result<Self> {
        // read config file to fetch existing environments
        let penv_path = home.join("penv.toml");
        let metadata = fs::metadata(&penv_path);

        let penv = if metadata.is_err() || !metadata.unwrap().is_file() {
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
            let penv_contents = fs::read_to_string(penv_path)?;
            toml::from_str(&penv_contents)?
        };

        tracing::debug!(environments=?penv.environments, installed_releases=?penv.cache.data.installed_releases, "created penv with environments");
        Ok(penv)
    }

    // TODO: delete this method and handle alternative repositories better
    pub fn new_from_repository(repository_name: String, home: Utf8PathBuf) -> Result<Self> {
        // read config file to fetch existing environments
        let penv_path = home.join("penv.toml");
        let metadata = fs::metadata(&penv_path);

        let penv = if metadata.is_err() || !metadata.unwrap().is_file() {
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
            let penv_contents = fs::read_to_string(penv_path)?;
            toml::from_str(&penv_contents)?
        };

        tracing::debug!(environments=?penv.environments, installed_releases=?penv.cache.data.installed_releases, "created penv with environments");
        Ok(penv)
    }

    /// Deactivate the current environment, removing any symlinks.
    pub fn deactivate(&mut self) -> Result<()> {
        self.active_environment = None;
        // Unset the symlink
        let link = self.home_dir.join("bin");
        let link_metadata = fs::metadata(link.clone());
        tracing::debug!("link_metadata: {:?}", link_metadata);
        if let Ok(link_metadata) = link_metadata {
            if link_metadata.is_symlink() || link_metadata.is_file() {
                tracing::debug!("removing symlink");
                fs::remove_file(link)?;
            } else if link_metadata.is_dir() {
                tracing::debug!("removing symlink");
                fs::remove_dir_all(link)?;
            }
        } else {
            tracing::debug!("symlink path {} does not exist", link);
        }
        self.persist()
    }

    pub fn delete_environment(&mut self, environment_alias: String) -> Result<()> {
        // Get the matching environment
        let environment = self.environments.get_environment(&environment_alias);
        if environment.is_none() {
            return Err(anyhow!(
                "Environment with alias {} does not exist",
                environment_alias
            ));
        }

        let environment = environment.unwrap();

        if self.active_environment == Some(environment.clone()) {
            return Err(anyhow!(
                "refusing to delete active environment {}; perhaps you mean to `penv deactivate` first",
                environment_alias
            ));
        }

        // Remove the environment from disk
        let env_path = &environment.metadata().root_dir;
        if env_path.exists() {
            tracing::debug!("removing environment directory: {}", env_path);
            std::fs::remove_dir_all(env_path)?;
        }

        // Remove the environment from the app
        self.environments
            .retain(|e| e.metadata().alias != environment_alias);

        // Persist the updated state
        self.persist()?;

        println!("deleted environment {}", environment_alias);

        Ok(())
    }

    pub fn reset_environment(
        &mut self,
        environment_alias: String,
        leave_client_state: bool,
        leave_node_state: bool,
    ) -> Result<()> {
        // Get the matching environment
        let environment = self.environments.get_environment(&environment_alias);
        if environment.is_none() {
            return Err(anyhow!(
                "Environment with alias {} does not exist",
                environment_alias
            ));
        }

        let environment = environment.unwrap();

        let env_path = &environment.path();

        // TODO: move this to an implementation on the binaries,
        // maybe a trait implementation.
        if !environment.metadata().client_only && !leave_node_state {
            // TODO: support multiple nodes
            // Why not use the `pd unsafe-reset-all` here?
            let node_dir = env_path.join("network_data").join("node0");
            let cometbft_data_dir = node_dir.join("cometbft").join("data");
            let pd_dir = node_dir.join("pd");

            let state_db_dir = cometbft_data_dir.join("state.db");
            if state_db_dir.exists() {
                tracing::debug!("removing data directory: {}", state_db_dir);
                std::fs::remove_dir_all(&state_db_dir)?;
            }

            let tx_index_db_dir = cometbft_data_dir.join("tx_index.db");
            if tx_index_db_dir.exists() {
                tracing::debug!("removing data directory: {}", tx_index_db_dir);
                std::fs::remove_dir_all(&tx_index_db_dir)?;
            }

            let blockstore_db_dir = cometbft_data_dir.join("blockstore.db");
            if blockstore_db_dir.exists() {
                tracing::debug!("removing data directory: {}", blockstore_db_dir);
                std::fs::remove_dir_all(&blockstore_db_dir)?;
            }

            let cs_wal_dir = cometbft_data_dir.join("cs.wal");
            if cs_wal_dir.exists() {
                tracing::debug!("removing data directory: {}", cs_wal_dir);
                std::fs::remove_dir_all(&cs_wal_dir)?;
            }

            let evidence_db_dir = cometbft_data_dir.join("evidence.db");
            if evidence_db_dir.exists() {
                tracing::debug!("removing data directory: {}", evidence_db_dir);
                std::fs::remove_dir_all(&evidence_db_dir)?;
            }

            let rocksdb_dir = pd_dir.join("rocksdb");
            if rocksdb_dir.exists() {
                tracing::debug!("removing data directory: {}", rocksdb_dir);
                std::fs::remove_dir_all(&rocksdb_dir)?;
            }

            let contents = "{}";
            let priv_validator_state = cometbft_data_dir.join("priv_validator_state.json");
            fs::write(priv_validator_state, contents)
                .context("Unable to write priv_validator_state.json")?;
        }

        if !leave_client_state {
            let pcli_bin = environment.get_pcli_binary();

            let pcli_args = vec![
                "--home".to_string(),
                pcli_bin.pcli_data_dir().to_string(),
                "view".to_string(),
                "reset".to_string(),
            ];
            // Execute the pcli binary with the given arguments
            tracing::debug!(path=?pcli_bin.path(), args=?pcli_args, "executing pcli binary");
            let output = Command::new(pcli_bin.path()).args(pcli_args).output()?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                tracing::debug!(?stdout, "command output");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("Command failed with error:\n{}", stderr));
            }
        }

        // Persist the updated state
        self.persist()?;

        println!("reset environment {}", environment_alias);

        Ok(())
    }

    pub fn create_environment(
        &mut self,
        environment_alias: String,
        penumbra_version: RepoOrVersionReq,
        grpc_url: Url,
        pd_join_url: Url,
        // eventually allow auto-download
        _repository_name: String,
        client_only: bool,
        generate_network: bool,
        import_seed_phrase: Option<String>,
    ) -> Result<Arc<Environment>> {
        if self
            .environments
            .iter()
            .any(|e| e.metadata().alias == environment_alias)
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

        match *matching_installed_version {
            InstalledRelease::GitCheckout(ref release) => {
                let root_dir = cache
                    .home
                    .join("environments")
                    .join(environment_alias.clone());

                // The cache's git checkout will be copied to the environment directory and then
                // it may be manually checked out to the desired state.
                let environment = Arc::new(Environment::CheckoutEnvironment(CheckoutEnvironment {
                    metadata: EnvironmentMetadata {
                        alias: environment_alias.clone(),
                        grpc_url: grpc_url.clone(),
                        root_dir,
                        client_only,
                        pd_join_url,
                        generate_network,
                    },
                    git_checkout: Arc::new(release.clone()),
                }));

                tracing::debug!("initializing environment");
                // Copy the checkout into the environment dir.
                environment.initialize_with_seed_phrase(cache, import_seed_phrase.clone())?;

                tracing::debug!("created environment: {:?}", environment);

                // Add a reference to the environment to the app
                self.environments.push(environment.clone());

                self.persist()?;

                Ok(environment)
            }
            InstalledRelease::Binary(ref matching_installed_version) => {
                let root_dir = cache
                    .home
                    .join("environments")
                    .join(environment_alias.clone());

                let (version_requirement, pinned_version) = (
                    match penumbra_version {
                        RepoOrVersionReq::Repo(_) => unreachable!(
                            "binary releases shouldn't return a repo installed version"
                        ),
                        RepoOrVersionReq::VersionReqOrLatest(version_req) => version_req,
                    },
                    matching_installed_version.version.clone(),
                );

                let environment = Arc::new(Environment::BinaryEnvironment(BinaryEnvironment {
                    metadata: EnvironmentMetadata {
                        alias: environment_alias.clone(),
                        grpc_url: grpc_url.clone(),
                        root_dir,
                        client_only,
                        pd_join_url,
                        generate_network,
                    },
                    version_requirement,
                    pinned_version,
                }));

                tracing::debug!("initializing environment");
                environment.initialize_with_seed_phrase(cache, import_seed_phrase.clone())?;

                tracing::debug!("created environment: {:?}", environment);

                // Add a reference to the environment to the app
                self.environments.push(environment.clone());

                self.persist()?;

                Ok(environment)
            }
        }
    }

    /// Returns all available versions and whether they're installed, optionally matching a given semver version requirement.
    pub async fn list_available(
        &self,
        required_version: Option<&RepoOrVersionReq>,
    ) -> Result<Vec<(Release, bool)>> {
        self.cache
            .list_available(required_version, &self.downloader)
            .await
    }

    pub async fn install_release(
        &mut self,
        penumbra_version: RepoOrVersionReq,
        target_arch: Triple,
    ) -> Result<()> {
        let installable_release = {
            let mut candidate_releases = Vec::new();

            match penumbra_version {
                // a Repo requirement will never meet a version returned from the binary release downloader
                // TODO: split downloader into a binary release downloader and git repo downloader
                RepoOrVersionReq::Repo(ref repo_url) => {
                    let installed_release = self
                        .cache
                        .get_installed_release(&RepoOrVersion::Repo(repo_url.clone()));

                    // TODO: actually use gix and try to validate the checkout
                    // let target_repo_dir_metadata = fs::metadata(target_repo_dir.clone());
                    if installed_release.is_some() {
                        tracing::debug!("have candidate {} installed", repo_url);
                        return Err(anyhow!("Git repo {} already installed", repo_url));
                    }

                    // Create a new InstallableRelease for this repo
                    // TODO: bad to have this here
                    let mut path = self.cache.home.join("checkouts");

                    let target_repo_dir =
                    // TODO: this will only allow a single checkout of a given repo url,
                    // there should maybe be a nonce or index or something to allow multiple checkouts
                    hex::encode(Sha256::digest(repo_url.to_string().as_bytes()));
                    path.push(target_repo_dir.clone());
                    Ok(InstallableRelease::GitRepo(RepoMetadata {
                        // TODO: a different name?
                        name: repo_url.clone(),
                        url: repo_url.clone(),
                        checkout_dir: target_repo_dir.into(),
                    }))
                }
                RepoOrVersionReq::VersionReqOrLatest(ref penumbra_version) => {
                    let downloader = &self.downloader;
                    let releases = downloader.fetch_releases().await?;
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

                    let cache = &mut self.cache;

                    // Skip installation if the installed_releases already contains this release
                    if cache.data.installed_releases.iter().any(|r| {
                        match *r {
                            InstalledRelease::Binary(ref r) => r.version == latest_release.version,
                            // TODO: implement for git checkouts
                            InstalledRelease::GitCheckout(_) => false,
                        }
                    }) {
                        println!("release {} already installed", latest_release.version);
                        return Ok(());
                    }

                    println!(
                        "downloading latest matching release: {}",
                        latest_release.version
                    );
                    downloader
                        .download_release(latest_release, format!("{}", target_arch))
                        .await
                }
            }
        }?;

        tracing::debug!("installable release prepared: {:?}", installable_release);

        // 5. attempt to install to the cache
        println!(
            "installing latest matching release: {}",
            installable_release
        );
        self.cache.install_release(&installable_release)?;

        self.persist()?;

        Ok(())
    }

    pub fn penv_file_path(&self) -> Utf8PathBuf {
        self.home_dir.join("penv.toml")
    }

    pub fn persist(&self) -> Result<()> {
        fs::create_dir_all(&self.home_dir)
            .with_context(|| format!("Failed to create home directory {}", self.home_dir))?;

        let toml_penv = toml::to_string(&self)?;

        tracing::debug!(penv_file_path=?self.penv_file_path(),"create file");

        let mut file = File::create(self.penv_file_path())?;
        file.write_all(toml_penv.as_bytes())?;

        tracing::debug!("persist cache");
        self.cache.persist()?;

        Ok(())
    }

    pub fn environment_info(&self, environment_alias: String) -> Result<&Environment> {
        let environment = self
            .environments
            .iter()
            .find(|e| e.metadata().alias == environment_alias)
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
            .find(|e| e.metadata().alias == environment_alias)
            .ok_or_else(|| anyhow!("Environment with alias {} not found", environment_alias))?;

        self.active_environment = Some(environment.clone());

        // Symlink the active environment's bin directory
        create_symlink(
            &environment.clone().path().join("bin"),
            &self.home_dir.join("bin"),
        )
        .context("error creating pcli symlink")?;

        Ok(environment)
    }

    pub fn replace_version(
        &mut self,
        environment_alias: String,
        new_version: Version,
    ) -> Result<()> {
        let mut environment = self
            .environments
            .iter()
            .find(|e| e.metadata().alias == environment_alias)
            .ok_or_else(|| anyhow!("Environment with alias {} not found", environment_alias))?
            .clone();

        let e = Arc::make_mut(&mut environment);

        match *e {
            Environment::BinaryEnvironment(ref mut e) => {
                e.pinned_version = new_version;
            }
            Environment::CheckoutEnvironment(_) => {
                return Err(anyhow!("Cannot replace version for a checkout environment"));
            }
        }

        let e = Arc::new(e.clone());

        // Remove the environment from the environment list
        self.environments
            .retain(|e| e.metadata().alias != environment_alias);

        // Update the stored environment within penv
        if let Some(active) = &self.active_environment {
            if active.as_ref().metadata().alias == environment_alias {
                self.active_environment = Some(e.clone());
            }
        }

        self.environments.push(e.clone());

        self.persist()?;

        environment.remove_symlinks()?;
        environment.create_symlinks(&self.cache)?;

        Ok(())
    }

    pub fn path_string(&self) -> String {
        self.home_dir.join("bin").to_string()
    }

    pub fn pcli_home(&self) -> Option<Utf8PathBuf> {
        self.active_environment
            .as_ref()
            .map(|environment| environment.path().join("pcli"))
    }

    pub fn pclientd_home(&self) -> Option<Utf8PathBuf> {
        self.active_environment
            .as_ref()
            .map(|environment| environment.path().join("pclientd"))
    }

    pub fn pd_home(&self) -> Option<Utf8PathBuf> {
        self.active_environment.as_ref().map(|environment| {
            environment
                .path()
                .join("network_data")
                .join("node0")
                .join("pd")
        })
    }

    // TODO: move to Environment trait
    pub fn cometbft_home(&self) -> Option<Utf8PathBuf> {
        self.active_environment.as_ref().map(|environment| {
            environment
                .path()
                .join("network_data")
                .join("node0")
                .join("cometbft")
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use semver::Version;
    use target_lexicon::Triple;

    use crate::penv::{
        cache::cache::CacheData,
        release::{
            binary::InstalledBinaryRelease, git_repo::CheckoutMetadata, InstalledAsset,
            VersionReqOrLatest,
        },
    };

    use super::*;

    #[test]
    fn deserialize_penv() {
        let cache_data = CacheData {
            installed_releases: vec![
                InstalledRelease::GitCheckout(CheckoutMetadata {
                    name: "test".into(),
                    url: "http://localhost:50051".into(),
                    install_path: "/tmp/test".into(),
                }),
                InstalledRelease::Binary(InstalledBinaryRelease {
                    version: Version::parse("1.0.0").unwrap(),
                    body: Some("Release notes for version 1.0.0".to_string()),
                    assets: vec![InstalledAsset {
                        target_arch: Triple::from_str("x86_64-unknown-linux-gnu").unwrap(),
                        local_filepath: Utf8PathBuf::from("/tmp/fake"),
                    }],
                    name: "Release 1.0.0".to_string(),
                    root_dir: Utf8PathBuf::from("/tmp/fake"),
                }),
            ],
        };
        let penv = Penv {
            cache: Cache {
                data: cache_data,
                home: "/tmp/test".into(),
            },
            downloader: Downloader::new("test/test".into()).expect("test downloader"),
            repository_name: "test".into(),
            home_dir: "/tmp/test".into(),
            active_environment: Some(Arc::new(Environment::CheckoutEnvironment(
                CheckoutEnvironment {
                    metadata: EnvironmentMetadata {
                        alias: "test".into(),
                        grpc_url: Url::parse("http://localhost:50051").unwrap(),
                        root_dir: "/tmp/test".into(),
                        client_only: false,
                        pd_join_url: Url::parse("http://localhost:50051").unwrap(),
                        generate_network: false,
                    },
                    git_checkout: Arc::new(CheckoutMetadata {
                        name: "test".into(),
                        url: "http://localhost:50051".into(),
                        install_path: "/tmp/test".into(),
                    }),
                },
            ))),
            environments: Environments {
                environments: vec![
                    Arc::new(Environment::CheckoutEnvironment(CheckoutEnvironment {
                        metadata: EnvironmentMetadata {
                            alias: "test".into(),
                            grpc_url: Url::parse("http://localhost:50051").unwrap(),
                            root_dir: "/tmp/test".into(),
                            client_only: false,
                            pd_join_url: Url::parse("http://localhost:50051").unwrap(),
                            generate_network: false,
                        },
                        git_checkout: Arc::new(CheckoutMetadata {
                            name: "test".into(),
                            url: "http://localhost:50051".into(),
                            install_path: "/tmp/test".into(),
                        }),
                    })),
                    Arc::new(Environment::BinaryEnvironment(BinaryEnvironment {
                        metadata: EnvironmentMetadata {
                            alias: "test".into(),
                            grpc_url: Url::parse("http://localhost:50051").unwrap(),
                            root_dir: "/tmp/test".into(),
                            client_only: false,
                            pd_join_url: Url::parse("http://localhost:50051").unwrap(),
                            generate_network: false,
                        },
                        version_requirement: VersionReqOrLatest::Latest,
                        pinned_version: Version::parse("1.0.0").unwrap(),
                    })),
                ],
            },
        };

        // Serialize to TOML string
        eprintln!("{}", toml::to_string(&penv).unwrap());

        // Example TOML string for deserialization
        let toml_str = r#"
            repository_name = "test"
            home_dir = "/tmp/test"
            active_environment = "test"
            [[cache.installed_releases]]
            type = "GitCheckout"

            [cache.installed_releases.args]
            name = "test"
            url = "http://localhost:50051"
            install_path = "/tmp/test"

            [[cache.installed_releases]]
            type = "Binary"

            [cache.installed_releases.args]
            version = "1.0.0"
            body = "Release notes for version 1.0.0"
            name = "Release 1.0.0"
            root_dir = "/tmp/fake"

            [[cache.installed_releases.args.assets]]
            target_arch = "x86_64-unknown-linux-gnu"
            local_filepath = "/tmp/fake"
            [[environments.environments]]
            type = "CheckoutEnvironment"
            [environments.environments.args.metadata]
            alias = "test"
            grpc_url = "http://localhost:50051/"
            root_dir = "/tmp/test"
            pd_join_url = "http://localhost:50051/"
            client_only = false
            generate_network = false

            [environments.environments.args.git_checkout]
            name = "test"
            url = "http://localhost:50051"
            install_path = "/tmp/test"

            [[environments.environments]]
            type = "BinaryEnvironment"

            [environments.environments.args]
            pinned_version = "1.0.0"

            [environments.environments.args.version_requirement]
            type = "Latest"

            [environments.environments.args.metadata]
            alias = "test"
            grpc_url = "http://localhost:50051/"
            root_dir = "/tmp/test"
            pd_join_url = "http://localhost:50051/"
            client_only = false
            generate_network = false
        "#;

        // Deserialize from TOML string
        let penv = toml::from_str::<Penv>(toml_str).unwrap();
        assert!(penv.environments.len() == 2);
        assert!(penv.active_environment.is_some());
    }
}

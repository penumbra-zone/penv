use anyhow::Context as _;
use anyhow::{Error, Result};
use gix::clone;
use gix::open;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;

// TODO: expose as method on Downloader...
pub fn clone_repo(repo_url: &str, dest: &str) -> Result<()> {
    let repo = if repo_url.starts_with("http") || repo_url.starts_with("git@") {
        let kind = gix::create::Kind::WithWorktree;
        let create_opts = gix::create::Options::default();
        let open_opts = gix::open::Options::default();
        fs::create_dir_all(dest)
            .with_context(|| format!("Failed to create dest directory {}", dest))?;
        let mut prep = clone::PrepareFetch::new(repo_url, dest, kind, create_opts, open_opts)?;
        // let mut progress_bar = ProgressBar::new(0);
        // progress_bar.set_style(ProgressStyle::default_bar()
        //         .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        //         .progress_chars("#>-"));
        println!("fetch...");
        let (mut prepare_checkout, _) =
            prep.fetch_then_checkout(gix::progress::Discard, &false.into())?;
        println!(
            "Checking out into {:?} ...",
            prepare_checkout.repo().work_dir().expect("should be there")
        );
        let (repo, _) = prepare_checkout
            .main_worktree(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)?;
        println!(
            "Repo cloned into {:?}",
            repo.work_dir().expect("directory pre-created")
        );

        let remote = repo
            .find_default_remote(gix::remote::Direction::Fetch)
            .expect("always present after clone")?;

        println!(
            "Default remote: {} -> {}",
            remote
                .name()
                .expect("default remote is always named")
                .as_bstr(),
            remote
                .url(gix::remote::Direction::Fetch)
                .expect("should be the remote URL")
                .to_bstring(),
        );
    } else {
        // If it's a local path, just use a symlink
        unimplemented!("local git repo support not implemented yet");
    };
    println!("Repository cloned to: {}", dest);
    Ok(())
}

use anyhow::{anyhow, Context, Result};
use ignore::gitignore::{self, Gitignore};
use notify::{watcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime};
use tracing::error;

use crate::global_config::read_global_config;
use crate::worker_client::push_to_worker;

pub async fn summon_daemon(global_config_dir: PathBuf) -> Result<()> {
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(10)).unwrap();

    let config = read_global_config(global_config_dir).context("Failed to read global config")?;
    if config.repos.is_none() {
        return Err(anyhow!(
            "You need to attach a repo before starting the Build Recall Daemon."
        ));
    }

    let mut ignores: HashMap<PathBuf, Gitignore> = HashMap::new();

    for repo in config.repos.clone().unwrap_or(vec![]) {
        let (gi, err) = Gitignore::new(repo.path.clone().join(".gitignore"));
        if err.is_some() {
            return Err(err.unwrap().into()); // fails if there's no git ignore
        }

        ignores.insert(repo.path.clone(), gi);
        watcher
            .watch(repo.path.clone(), RecursiveMode::Recursive)
            .unwrap();
    }

    let mut now = SystemTime::now();

    loop {
        match rx.recv() {
            Ok(event) => {
                let path = match event {
                    notify::DebouncedEvent::NoticeWrite(p) => p,
                    notify::DebouncedEvent::NoticeRemove(p) => p,
                    notify::DebouncedEvent::Create(p) => p,
                    notify::DebouncedEvent::Write(p) => p,
                    notify::DebouncedEvent::Chmod(p) => p,
                    notify::DebouncedEvent::Remove(p) => p,
                    notify::DebouncedEvent::Rename(_, p) => p,
                    notify::DebouncedEvent::Rescan => continue,
                    notify::DebouncedEvent::Error(e, _) => {
                        tracing::error!("{:?}", e);
                        continue;
                    }
                };

                let repo = config
                    .repos
                    .clone()
                    .unwrap_or(vec![])
                    .iter()
                    .map(|r| r.path.clone())
                    .find(|c| path.clone().starts_with(c));

                // This would mean that we're getting events for a repo we shouldn't
                // be watching; so we should consider this an error.
                if repo.is_none() {
                    return Err(anyhow!(
                        "Somehow receiving events for a repo we didn't intend to watch: {:?}",
                        path.clone()
                    ));
                }

                let repo_path = &repo.unwrap();
                let maybe_ig = ignores.get(&repo_path.clone());

                // if there's no gitignore, we should error
                if maybe_ig.is_none() {
                    return Err(anyhow!(
                        "Unexpectedly did not find a gitignore at {:?}",
                        repo_path.clone()
                    ));
                }

                let relative = path.strip_prefix(repo_path)?;
                let ig = maybe_ig.unwrap();
                if ig
                    .matched_path_or_any_parents(&relative.clone(), false)
                    .is_ignore()
                    == false
                {
                    // Run the build!
                    println!("build triggered by {:?} {:?}", relative, repo_path);
                    push_to_worker(repo_path.clone()).await?;
                }
            }
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}

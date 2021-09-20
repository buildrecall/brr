use std::{
    fs,
    path::{self, Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use git2::{IndexAddOption, PushOptions, RemoteCallbacks};
use uuid::Uuid;

use crate::{global_config::read_global_config, worker_client::init_git_transport};

fn copy_folder<U: AsRef<Path>, V: AsRef<Path>>(from: U, to: V) -> Result<()> {
    let mut stack = Vec::new();
    stack.push(PathBuf::from(from.as_ref()));

    let output_root = PathBuf::from(to.as_ref());
    let input_root = PathBuf::from(from.as_ref()).components().count();

    while let Some(working_path) = stack.pop() {
        let src: PathBuf = working_path.components().skip(input_root).collect();

        // Create a destination if missing
        let dest = if src.components().count() == 0 {
            output_root.clone()
        } else {
            output_root.join(&src)
        };
        if fs::metadata(&dest).is_err() {
            fs::create_dir_all(&dest)?;
        }

        for entry in fs::read_dir(working_path.clone())
            .context(format!("Failed to read dir {:?}", working_path))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                match path.file_name() {
                    Some(filename) => {
                        let dest_path = dest.join(filename);
                        fs::copy(&path, &dest_path).context(format!(
                            "Failed to copy file from {:?} to {:?}",
                            path, dest_path
                        ))?;
                    }
                    None => return Err(anyhow!("failed to read file_name: {:?}", path)),
                }
            }
        }
    }

    Ok(())
}

fn worktree_path(global_config_dir: PathBuf, project_id: uuid::Uuid) -> Result<PathBuf> {
    let config = read_global_config(global_config_dir.clone())?;
    let repoconfig = config
        .repo_config_by_id(project_id)
        .ok_or(anyhow!("No project in config with id: {}", project_id))?;

    Ok(path::Path::new(&repoconfig.path).to_path_buf())
}

fn repo_path(global_config_dir: PathBuf, project_id: uuid::Uuid) -> Result<PathBuf> {
    Ok(global_config_dir.join(".gits").join(project_id.to_string()))
}

pub struct RecallGit {
    global_config_dir: PathBuf,
}

impl RecallGit {
    pub fn new(global_config_dir: PathBuf) -> Result<RecallGit> {
        tracing_subscriber::fmt::init();
        init_git_transport();

        Ok(RecallGit {
            global_config_dir: global_config_dir,
        })
    }

    pub fn create_shadow_git_folder(&self, project_id: uuid::Uuid) -> Result<()> {
        // Create the .git
        let new_path = repo_path(self.global_config_dir.clone(), project_id)?;
        let repo = git2::Repository::init_bare(new_path)?;

        Ok(())
    }

    pub async fn push_project(&self, project_id: Uuid) -> Result<()> {
        let config = read_global_config(self.global_config_dir.clone())?;
        let dot_git_path = repo_path(self.global_config_dir.clone(), project_id.clone())
            .context("Failed to create path")?;

        let repo_exists = Path::new(&dot_git_path).is_dir();
        let repo = match repo_exists {
            true => git2::Repository::open(&dot_git_path)
                .context(format!("Failed to open repo {:?}", dot_git_path))?,
            false => git2::Repository::init_bare(dot_git_path.as_path())
                .context("Failed to init repo")?,
        };

        repo.set_workdir(
            &worktree_path(self.global_config_dir.clone(), project_id)?,
            false,
        )?;

        Ok(tokio::runtime::Handle::current()
            .spawn_blocking(move || -> Result<_> {
                let mut i = repo.index().context("Failed to get a git index")?;

                let tree = {
                    i.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
                    i.write_tree()
                }?;

                let tree = repo
                    .find_tree(tree)
                    .context("Failed to find a git tree in this repository")?;
                let sig = repo.signature().context(
                "failed to create a git signature (needed to make a commit in the shadow git repo)",
            )?;

                repo.commit(None, &sig, &sig, "sync with buildrecall", &tree, &[])
                    .context("Failed to commit to the shadow git project")?;

                let mut push_cbs = RemoteCallbacks::new();
                push_cbs.push_update_reference(|ref_, msg| {
                    eprintln!("{:?}", (ref_, msg));
                    Ok(())
                });

                let remote_url = format!("{}/push", config.scheduler_host());
                let mut push_opts = PushOptions::new();
                push_opts.remote_callbacks(push_cbs);
                let mut remote = repo
                    .remote_anonymous(remote_url.clone().as_str())
                    .context("Failed to create an anonymous remote in the shadow git project")?;

                //  push to non-main branch so that we dont get "branch is currently checked out" error
                //  https://stackoverflow.com/questions/2816369/git-push-error-remote-rejected-master-master-branch-is-currently-checked
                let refspecs: &[&str] = &["+HEAD:refs/heads/incoming"];
                remote
                    .push(refspecs, Some(&mut push_opts))
                    .context(format!(
                        "Failed to push to the shadow git project with remote: {}",
                        remote_url
                    ))?;

                Ok(())
            })
            .await
            .context("Failed to spawn the tokio runtime")??)
    }
}

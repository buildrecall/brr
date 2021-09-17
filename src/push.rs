use anyhow::{anyhow, Context, Result};
use git2::{IndexAddOption, RepositoryInitOptions};
use std::{
    env,
    path::{self, Path, PathBuf},
};

use crate::{
    git::{repo_path, worktree_path},
    global_config::read_global_config,
    worker_client::{init, init_git_transport},
};

pub async fn run_push(global_config_dir: PathBuf) -> Result<()> {
    use git2::{PushOptions, RemoteCallbacks};

    init()?;

    //  push to non-main branch so that we dont get "branch is currently checked out" error
    //  https://stackoverflow.com/questions/2816369/git-push-error-remote-rejected-master-master-branch-is-currently-checked
    let refspecs: &[&str] = &["+HEAD:refs/heads/incoming"];

    let config = read_global_config(global_config_dir.clone())?;
    let repoconfig = config
        .clone()
        .repo_config_of_current_dir()?
        .ok_or(anyhow!(
            "This isn't a buildrecall project, run attach first."
        ))?
        .clone();

    let dot_git_path =
        repo_path(global_config_dir.clone(), repoconfig.id).context("Failed to create path")?;
    let repo_exists = Path::new(&dot_git_path).is_dir();
    let repo = match repo_exists {
        true => git2::Repository::open(&dot_git_path)
            .context(format!("Failed to open repo {:?}", dot_git_path))?,
        false => {
            let r = git2::Repository::init_bare(dot_git_path.as_path())
                .context("Failed to init repo")?;
            r.set_workdir(&worktree_path(global_config_dir, repoconfig.id)?, false)?;
            r
        }
    };

    Ok(tokio::runtime::Handle::current()
        .spawn_blocking(move || {
            let mut i = repo.index()?;

            i.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
            i.write()?;

            let mut push_cbs = RemoteCallbacks::new();
            push_cbs.push_update_reference(|ref_, msg| {
                eprintln!("{:?}", (ref_, msg));
                Ok(())
            });

            let mut push_opts = PushOptions::new();
            push_opts.remote_callbacks(push_cbs);
            let mut remote =
                repo.remote_anonymous(format!("{}/push", config.scheduler_host()).as_str())?;

            remote.push(refspecs, Some(&mut push_opts))
        })
        .await
        .context("Failed to spawn the tokio runtime")??)
}

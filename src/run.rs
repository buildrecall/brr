use anyhow::{anyhow, Context, Result};
use std::{env, path::PathBuf};

use crate::{
    api::{ApiClient, BuildRecall, Project, PullQueryParams, PushQueryParams},
    config_global::read_global_config,
    config_local::read_local_config,
    git,
    push::run_push_in_current_dir_retry,
};

pub async fn preattach_to_repo(global_config_dir: PathBuf, slug: String) -> Result<uuid::Uuid> {
    let global_config = read_global_config(global_config_dir.clone()).context(format!(
        "Failed to read the global config file: {:?}",
        global_config_dir
    ))?;
    let client = ApiClient::new(global_config.clone());
    if global_config.clone().access_token().is_none() {
        return Err(anyhow!(
            "You're not logged in. You can login by going to https://buildrecall.com/setup"
        ));
    }

    let path = env::current_dir()?;

    let projects = client.list_projects().await?;
    let maybe_project = projects.iter().find(|p| p.slug == slug.clone());
    let project_res: Result<Project> = match maybe_project {
        Some(p) => Ok(p.clone()),
        None => {
            let p = client
                .create_project(slug.clone())
                .await
                .context("Failed to create the project in Buildrecall")?
                .clone();
            Ok(p)
        }
    };
    let project = project_res?;

    // create a .git folder for brr to use that doesn't mess with the user's git.
    let g = git::RecallGit::new(global_config_dir.clone())?;
    g.create_shadow_git_folder(slug.clone())
        .context(format!("Failed to create a shadow git folder (used to sync files without messing with your own git setup) in {:?}/{}", global_config_dir, ".gits"))?;

    Ok(project.id)
}

#[derive(Clone)]
pub struct JobArgs {
    pub job: String,
    pub container: String,
}

pub async fn run_pull(
    global_config_dir: PathBuf,
    current_dir: PathBuf,
    slug: String,
    args: JobArgs,
) -> Result<bool> {
    let config = read_global_config(global_config_dir.clone())
        .context("Failed to parse the global config ~/.builrecall/config.toml")?;
    let g = git::RecallGit::new(global_config_dir.clone())
        .context("Failed to create a shadow git instance")?;

    let local_config = read_local_config(git::worktree_path(slug.clone())?)?;
    let image = match local_config.containers.get(&args.container) {
        Some(c) => c.image.clone(),
        None => {
            anyhow::bail!("No image for container named {}", args.container);
        }
    };

    let oid = g
        .hash_folder(slug.clone())
        .await
        .context("Failed to hash this folder as a project")?;

    let client = ApiClient::new(config);

    let args = PullQueryParams {
        tree_hash: oid.to_string(),
        project_slug: slug,
        job: args.job.clone(),
        container: args.container.clone(),
        image,
    };

    let pulled = client
        .pull_project(args)
        .await
        .context("Failed to pull project")?;

    Ok(pulled)
}

pub async fn pull_with_push_if_needed(
    global_config_dir: PathBuf,
    current_dir: PathBuf,
    args: JobArgs,
) -> Result<()> {
    let local =
        read_local_config(current_dir.clone()).context("Failed to read buildrecall.toml")?;
    let slug = local.project().name.ok_or(anyhow!(
        "buildrecall.toml is missing a 'project.name' field"
    ))?;

    preattach_to_repo(global_config_dir.clone(), slug.clone())
        .await
        .context(format!(
            "Failed to attach the project '{}' to this folder",
            slug
        ))?;

    let mut pulled = run_pull(
        global_config_dir.clone(),
        current_dir.clone(),
        slug.clone(),
        args.clone(),
    )
    .await?;

    if !pulled {
        run_push_in_current_dir_retry(global_config_dir.clone(), slug.clone(), args.clone())
            .await?;
        pulled = run_pull(
            global_config_dir.clone(),
            current_dir,
            slug.clone(),
            args.clone(),
        )
        .await?;
    }

    if !pulled {
        return Err(anyhow!("buildrecall artifacts unavailable for this build"));
    }

    Ok(())
}

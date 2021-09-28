use anyhow::{anyhow, Context, Result};
use std::{env, path::PathBuf};

use crate::{
    api::{ApiClient, BuildRecall, Project},
    config_global::{overwrite_global_config, read_global_config, GlobalConfig, RepoConfig},
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
    let existing = global_config.repo_config_of_pathbuf(path.clone())?;
    if existing.is_some() {
        return Ok(existing.unwrap().id); // skip if we already have something in our config
    }

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

    // create the project in the local config
    let project_config_id = project.clone().id.clone();
    overwrite_global_config(global_config_dir.clone(), move |c| {
        let mut repos: Vec<RepoConfig> = vec![RepoConfig {
            path: path,
            name: slug.clone(),
            id: project_config_id,
        }];
        repos.extend(c.repos.unwrap_or(vec![]));

        GlobalConfig {
            connection: c.connection,
            repos: Some(repos),
        }
    })
    .context("Failed to store this project in the global config file")?;

    // create a .git folder for brr to use that doesn't mess with the user's git.
    let g = git::RecallGit::new(global_config_dir.clone())?;
    g.create_shadow_git_folder(project_config_id)
        .context(format!("Failed to create a shadow git folder (used to sync files without messing with your own git setup) in {:?}/{}", global_config_dir, ".gits"))?;

    Ok(project.id)
}

pub async fn run_pull(
    global_config_dir: PathBuf,
    current_dir: PathBuf,
    project_id: uuid::Uuid,
) -> Result<bool> {
    let config = read_global_config(global_config_dir.clone())
        .context("Failed to parse the global config ~/.builrecall/config.toml")?;
    let g = git::RecallGit::new(global_config_dir.clone())
        .context("Failed to create a shadow git instance")?;

    let oid = g
        .hash_folder(project_id)
        .await
        .context("Failed to hash this folder as a project")?;

    let client = ApiClient::new(config);

    let pulled = client
        .pull_project(project_id, oid.to_string())
        .await
        .context("Failed to pull project")?;

    Ok(pulled)
}

pub async fn pull_with_push_if_needed(
    global_config_dir: PathBuf,
    current_dir: PathBuf,
    job: String,
) -> Result<()> {
    let local =
        read_local_config(current_dir.clone()).context("Failed to read buildrecall.toml")?;
    let slug = local.project().name.ok_or(anyhow!(
        "buildrecall.toml is missing a 'project.name' field"
    ))?;

    let project_id = preattach_to_repo(global_config_dir.clone(), slug.clone())
        .await
        .context(format!(
            "Failed to attach the project '{}' to this folder",
            slug
        ))?;

    let mut pulled = run_pull(global_config_dir.clone(), current_dir.clone(), project_id).await?;

    if !pulled {
        run_push_in_current_dir_retry(global_config_dir.clone(), project_id).await?;
        pulled = run_pull(global_config_dir.clone(), current_dir, project_id).await?;
    }

    if !pulled {
        return Err(anyhow!("buildrecall artifacts unavailable for this build"));
    }

    Ok(())
}

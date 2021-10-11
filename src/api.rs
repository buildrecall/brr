use std::{fs, io::Read};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use futures::TryFutureExt;
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Something is wrong with your access token, perhaps you've been logged out by the server? You can login again at https://buildrecall.com/setup")]
    Unauthorized,
    #[error("Failed to connect to Build Recall at {host:?}. Perhaps your internet is down, or Build Recall is having an outage.\n\n{err:?}")]
    FailedToConnect { host: String, err: reqwest::Error },
    #[error("Failed to '{request:?}' got a status code of: {status:?})")]
    BadResponse { status: StatusCode, request: String },
}

use crate::{config_global::GlobalConfig, git::PushQueryParams};

#[derive(Serialize, Deserialize)]
pub struct LoginRequestBody {
    pub single_use_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LoginRequestResponseBody {
    // The "real" access token that's saved on the user's
    // computer and used to auth
    pub access_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Project {
    pub id: uuid::Uuid,

    // Aka the "name", what's used in the command line arg.
    // This can be changed by a user
    pub slug: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateProjectBody {
    pub slug: String,
}

// Eventually, we'll use this body to let folks
// add "approved" email domains, but for the moment
// it is empty
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateInviteBody {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrgInvite {
    pub org_id: uuid::Uuid,
    pub token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectSecret {
    pub id: uuid::Uuid,
    pub created_at: chrono::DateTime<Utc>,
    pub project_id: uuid::Uuid,
    pub created_by: uuid::Uuid,
    pub slug: String,
    pub version: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PullResponse {
    // A pre-signed S3 URL
    pub artifact_url: Option<String>,
    pub logs_url: String,
}

#[async_trait]
pub trait BuildRecall {
    async fn login(&self, body: LoginRequestBody) -> Result<LoginRequestResponseBody>;
    async fn list_projects(&self) -> Result<Vec<Project>>;
    async fn create_project(&self, slug: String) -> Result<Project>;
    async fn invite(&self) -> Result<OrgInvite>;
    //  returns whether artifact were ready
    async fn pull_project(&self, slug: String, args: PushQueryParams, hash: String)
        -> Result<bool>;
    async fn set_secret(
        &self,
        project_slug: String,
        secret_slug: String,
        value: String,
    ) -> Result<ProjectSecret>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiClient {
    global_config: GlobalConfig,
}

impl ApiClient {
    pub fn new(global_config: GlobalConfig) -> ApiClient {
        ApiClient { global_config }
    }

    pub fn get_control_host(&self) -> String {
        self.global_config.clone().control_host()
    }

    pub fn get_scheduler_host(&self) -> String {
        self.global_config.clone().scheduler_host()
    }

    fn token(&self) -> Result<String> {
        self.global_config.clone().access_token()
        .ok_or(
            anyhow!("Can't find an 'access_token'. Specify one in your global config file (which typically lives at ~/.buildrecall/config) or if in CI, in the BUILDRECALL_API_KEY env var."))
    }

    async fn pull_artifact_url(
        &self,
        slug: String,
        args: &PushQueryParams,
        hash: String,
    ) -> Result<PullResponse> {
        let client = reqwest::Client::new();

        let tok = self.token()?.clone();
        let scheduler_host = self.get_scheduler_host().clone();

        let pullresp = client
            .get(format!(
                "{}/p/{}/pull/{}",
                scheduler_host.clone(),
                slug.clone(),
                &hash
            ))
            .query(args)
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| ApiError::FailedToConnect {
                host: scheduler_host.clone(),
                err: e,
            })?;

        if pullresp.status() == 401 {
            return Err(ApiError::Unauthorized.into());
        }

        if !pullresp.status().is_success() {
            return Err(ApiError::BadResponse {
                request: format!("GET {}/p/{}/pull/{}", scheduler_host, slug.clone(), &hash),
                status: pullresp.status(),
            }
            .into());
        }

        let r = pullresp.json::<PullResponse>().await?;

        Ok(r)
    }
}

#[async_trait]
impl BuildRecall for ApiClient {
    async fn login(&self, body: LoginRequestBody) -> Result<LoginRequestResponseBody> {
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/cli/login", self.get_control_host()))
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::FailedToConnect {
                host: self.get_control_host(),
                err: e,
            })?;

        if resp.status() == 401 {
            return Err(anyhow!(
                    "The token '{}' is expired, invalid, or has already been used to login.\nYou can get a new one at https://buildrecall.com/setup",
                    body.single_use_token
                ));
        }
        if !resp.status().is_success() {
            return Err(ApiError::BadResponse {
                request: format!("POST {}/v1/cli/login", self.get_control_host()),
                status: resp.status(),
            }
            .into());
        }
        let result = resp.json::<LoginRequestResponseBody>()
                .await
                .context("Failed to login to Build Recall. The response unexpectedly did not return a JSON body. This is almost certainly a bug in Build Recall. :(")?;

        Ok(result)
    }

    async fn set_secret(
        &self,
        project_slug: String,
        secret_slug: String,
        value: String,
    ) -> Result<ProjectSecret> {
        let client = reqwest::Client::new();
        #[derive(Deserialize, Serialize, Clone, Debug)]
        pub struct SetSecret {
            slug: String,
            value: String,
        }

        let tok = self.token()?;

        let resp = client
            .post(format!(
                "{}/v1/cli/projects/{}/secrets",
                self.get_control_host(),
                project_slug.clone()
            ))
            .json(&SetSecret {
                slug: secret_slug,
                value,
            })
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| ApiError::FailedToConnect {
                host: self.get_control_host(),
                err: e,
            })?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized.into());
        }
        if !resp.status().is_success() {
            return Err(ApiError::BadResponse {
                request: format!(
                    "POST {}/v1/cli/projects/{}/secrets",
                    self.get_control_host(),
                    project_slug.clone()
                ),
                status: resp.status(),
            }
            .into());
        }

        let secret = resp.json::<ProjectSecret>()
                .await
                .context("Failed to create this project. The response unexpectedly did not return a JSON body. This is almost certainly a bug in Build Recall. :(")?;
        Ok(secret)
    }

    async fn pull_project(
        &self,
        slug: String,
        args: PushQueryParams,
        hash: String,
    ) -> Result<bool> {
        let handle = tokio::runtime::Handle::current();

        let pull = self
            .pull_artifact_url(slug, &args, hash)
            .await
            .context("Failed to pull s3 signed url for this artifact")?;

        let _ = reqwest::get(&pull.logs_url)
            .and_then(|r| async {
                if !r.status().is_success() {
                    return Ok(());
                }
                r.text().await.map(|logs| {
                    if pull.artifact_url.is_none() {
                        eprintln!("logs of previous failed build:");
                    }
                    eprintln!("{}", logs);
                })
            })
            .await;

        let artifact_url = match pull.artifact_url {
            Some(u) => u,
            None => return Ok(false),
        };

        handle
            .spawn_blocking(move || -> Result<bool> {
                let client = reqwest::blocking::ClientBuilder::new()
                    .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
                    .build()?;

                let resp = client
                    .get(artifact_url)
                    .send()
                    .context("Failed to pull the artifact from S3")?;

                let mut a = tar::Archive::new(resp);

                for entry in a
                    .entries()
                    .context("Failed to list entries of tar archive")?
                {
                    let mut file = entry.context("Can't process an entry of tar archive")?;

                    let mut buf = Vec::new();
                    file.read_to_end(&mut buf)?;

                    let path = file
                        .header()
                        .path()
                        .context("Failed to parse path of archived file")?
                        .clone();

                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)
                            .context(format!("Failed to create directory {:?}", parent))?;
                    }
                    fs::write(path.clone(), buf)
                        .context(format!("Failed to write to file at {:?}", path))?;
                }

                Ok(true)
            })
            .await?
    }

    async fn list_projects(&self) -> Result<Vec<Project>> {
        let client = reqwest::Client::new();
        let tok = self.token()?;

        let resp = client
            .get(format!("{}/v1/cli/projects", self.get_control_host()))
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| ApiError::FailedToConnect {
                host: self.get_control_host(),
                err: e,
            })?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized.into());
        }
        if !resp.status().is_success() {
            return Err(ApiError::BadResponse {
                request: format!("GET {}/v1/cli/projects", self.get_control_host()),
                status: resp.status(),
            }
            .into());
        }

        let projects = resp.json::<Vec<Project>>()
        .await
        .context(format!("Failed to list projects. The response to {} unexpectedly did not return a JSON body. This is almost certainly a bug in Build Recall. :(", "/v1/cli/projects"))?;

        Ok(projects)
    }

    async fn create_project(&self, slug: String) -> Result<Project> {
        let client = reqwest::Client::new();
        let tok = self.token()?;

        let resp = client
            .post(format!("{}/v1/cli/projects", self.get_control_host()))
            .json(&CreateProjectBody { slug: slug })
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| ApiError::FailedToConnect {
                host: self.get_control_host(),
                err: e,
            })?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized.into());
        }
        if !resp.status().is_success() {
            return Err(ApiError::BadResponse {
                request: format!("POST {}/v1/cli/projects", self.get_control_host()),
                status: resp.status(),
            }
            .into());
        }

        let result = resp.json::<Project>()
                .await
                .context("Failed to create this project. The response unexpectedly did not return a JSON body. This is almost certainly a bug in Build Recall. :(")?;

        Ok(result)
    }

    async fn invite(&self) -> Result<OrgInvite> {
        let client = reqwest::Client::new();
        let tok = self.token()?;

        let resp = client
            .post(format!("{}/v1/cli/invites", self.get_control_host()))
            .json(&CreateInviteBody {})
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| ApiError::FailedToConnect {
                host: self.get_control_host(),
                err: e,
            })?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized.into());
        }
        if !resp.status().is_success() {
            return Err(ApiError::BadResponse {
                request: format!("POST {}/v1/cli/invites", self.get_control_host()),
                status: resp.status(),
            }
            .into());
        }

        let result = resp.json::<OrgInvite>()
                .await
                .context("Failed to create this project. The response unexpectedly did not return a JSON body. This is almost certainly a bug in Build Recall. :(")?;

        Ok(result)
    }
}

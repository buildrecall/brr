use std::fmt::format;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Something is wrong with your access token, perhaps you've been logged out by the server? You can login again at https://buildrecall.com/setup")]
    Unauthorized,
    #[error("Failed to connect to Build Recall. Perhaps your internet is down, or Build Recall is having an outage.")]
    FailedToConnect,
    #[error("Failed to '{request:?}' got a status code of: {status:?})")]
    BadResponse { status: StatusCode, request: String },
}

const BUILD_RECALL_HOST: &str = "https://buildrecall.com";

use crate::global_config::{self, GlobalConfig};

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

#[async_trait]
pub trait BuildRecall {
    async fn login(&self, body: LoginRequestBody) -> Result<LoginRequestResponseBody>;
    async fn list_projects(&self) -> Result<Vec<Project>>;
    async fn create_project(&self, slug: String) -> Result<Project>;
    async fn invite(&self) -> Result<OrgInvite>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiClient {
    global_config: GlobalConfig,
}

impl ApiClient {
    pub fn new(global_config: GlobalConfig) -> ApiClient {
        ApiClient { global_config }
    }

    pub fn get_host(&self) -> String {
        self.global_config.clone().control_host()
    }

    fn token(&self) -> Result<String> {
        self.global_config.clone().access_token().ok_or(
            anyhow!("Can't find an 'access_token' in your global config file (which typically lives at ~/.buildrecall/config)."))
    }
}

#[async_trait]
impl BuildRecall for ApiClient {
    async fn login(&self, body: LoginRequestBody) -> Result<LoginRequestResponseBody> {
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{}/v1/cli/login", self.get_host()))
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::FailedToConnect)?;

        if resp.status() == 401 {
            return Err(anyhow!(
                    "The token '{}' is expired, invalid, or has already been used to login.\nYou can get a new one at https://buildrecall.com/setup",
                    body.single_use_token
                ));
        }
        if !resp.status().is_success() {
            return Err(ApiError::BadResponse {
                request: format!("POST {}/v1/cli/login", self.get_host()),
                status: resp.status(),
            }
            .into());
        }
        let result = resp.json::<LoginRequestResponseBody>()
                .await
                .context("Failed to login to Build Recall. The response unexpectedly did not return a JSON body. This is almost certainly a bug in Build Recall. :(")?;

        Ok(result)
    }

    async fn list_projects(&self) -> Result<Vec<Project>> {
        let client = reqwest::Client::new();
        let tok = self.token()?;

        let resp = client
            .get(format!("{}/v1/cli/projects", self.get_host()))
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| ApiError::FailedToConnect)?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized.into());
        }
        if !resp.status().is_success() {
            return Err(ApiError::BadResponse {
                request: format!("GET {}/v1/cli/projects", self.get_host()),
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
            .post(format!("{}/v1/cli/projects", self.get_host()))
            .json(&CreateProjectBody { slug: slug })
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| ApiError::FailedToConnect)?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized.into());
        }
        if !resp.status().is_success() {
            return Err(ApiError::BadResponse {
                request: format!("POST {}/v1/cli/projects", self.get_host()),
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
            .post(format!("{}/v1/cli/invites", self.get_host()))
            .json(&CreateInviteBody {})
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| ApiError::FailedToConnect)?;

        if resp.status() == 401 {
            return Err(ApiError::Unauthorized.into());
        }
        if !resp.status().is_success() {
            return Err(ApiError::BadResponse {
                request: format!("POST {}/v1/cli/invites", self.get_host()),
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

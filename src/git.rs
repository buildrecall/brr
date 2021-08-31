use anyhow::{anyhow, Context, Result};
use git2::{IndexAddOption, Oid, PushOptions, RemoteCallbacks, Repository};
use hyper::{
    header::{AUTHORIZATION, UPGRADE},
    http::uri::Scheme,
    Body, Client, StatusCode,
};
use std::{
    convert::TryFrom,
    path::{self, Path, PathBuf},
    sync::Once,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::*;
use uuid::Uuid;

use crate::config_global::get_global_config_dir;
use crate::config_global::read_global_config;

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

const RECALL_GIT_SCHEME_HTTP: &str = "recall+git";
const RECALL_GIT_SCHEME_HTTPS: &str = "recalls+git";

fn init_git_transport() {
    static INIT: Once = Once::new();

    INIT.call_once(move || unsafe {
        git2::transport::register(RECALL_GIT_SCHEME_HTTP, |remote| {
            git2::transport::Transport::smart(remote, false, RecallGitTransport)
        })
        .context("Failed to register the git transport")
        .unwrap();
        git2::transport::register(RECALL_GIT_SCHEME_HTTPS, |remote| {
            git2::transport::Transport::smart(remote, false, RecallsGitTransport)
        })
        .context("Failed to register the git transport")
        .unwrap();
    });
}

pub struct RecallGit {
    global_config_dir: PathBuf,
}

impl RecallGit {
    pub fn new(global_config_dir: PathBuf) -> Result<RecallGit> {
        let _ = tracing_subscriber::fmt::try_init();
        init_git_transport();

        Ok(RecallGit {
            global_config_dir: global_config_dir,
        })
    }

    pub fn create_shadow_git_folder(&self, project_id: uuid::Uuid) -> Result<()> {
        // Create the .git
        let new_path = repo_path(self.global_config_dir.clone(), project_id)?;
        std::fs::create_dir_all(&new_path)?;
        git2::Repository::init_bare(new_path)?;

        Ok(())
    }

    pub fn get_repo_by_project(&self, project_id: uuid::Uuid) -> Result<Repository> {
        let dot_git_path = repo_path(self.global_config_dir.clone(), project_id.clone())
            .context("Failed to create path")?;

        let repo_exists = Path::new(&dot_git_path).is_dir();
        let repo = match repo_exists {
            true => git2::Repository::open_bare(&dot_git_path)
                .context(format!("Failed to open repo {:?}", dot_git_path)),
            false => {
                git2::Repository::init_bare(dot_git_path.as_path()).context("Failed to init repo")
            }
        }
        .context("Failed to init or open the shadow git repo")?;

        repo.set_workdir(
            &worktree_path(self.global_config_dir.clone(), project_id)?,
            false,
        )
        .context("Failed to create a workdir for the shadow git repo")?;

        Ok(repo)
    }

    pub async fn hash_folder(&self, project_id: uuid::Uuid) -> Result<Oid> {
        let repo = self
            .get_repo_by_project(project_id)
            .context("Failed to get git repository")?;

        let mut i = repo.index().context("Failed to get a git index")?;

        let hash = {
            i.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
                .context(
                    "Failed to stage changes in shadow git repo (required to compute git hash)",
                )?;
            i.write_tree()
        }
        .context("Failed to hash git repository")?;

        Ok(hash)
    }

    pub async fn push_project(&self, project_id: Uuid, retry: bool) -> Result<()> {
        let config = read_global_config(self.global_config_dir.clone())?;

        let repo = self
            .get_repo_by_project(project_id)
            .context("Failed to get git repository")?;

        let handle = tokio::runtime::Handle::current();

        Ok(handle
            .spawn_blocking(move || -> Result<_> {
                let mut i = repo.index().context("Failed to get a git index")?;

                let tree = {
                    i.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
                        .context("Failed to index add all")?;
                    i.write_tree()
                }
                .context("failed to generate a git tree")?;

                let tree = repo
                    .find_tree(tree)
                    .context("Failed to find a git tree in this repository")?;
                let sig = git2::Signature::now("buildrecall", "bot@buildrecall.com")
                .context(
                    "failed to create a git signature (needed to make a commit in the shadow git repo)",
                )?;

                //  update HEAD so that push works correctly
                let head = repo.head().ok().map(|h| h.peel_to_commit().ok()).flatten();
                let parents = head.map(|h| vec![h]).unwrap_or(vec![]);
                let parents: Vec<&git2::Commit> = parents.iter().collect();
                repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    "sync with buildrecall",
                    &tree,
                    &parents,
                )
                .context("Failed to commit to the shadow git project")?;

                let mut push_cbs = RemoteCallbacks::new();
                push_cbs.push_update_reference(|ref_, msg| {
                    eprintln!("{:?}", (ref_, msg));
                    Ok(())
                });

                #[derive(serde::Serialize)]
                struct PushParams {
                    wait: Option<bool>,
                    tree_hash_hex: Option<String>,
                }

                let query = serde_qs::to_string(&PushParams{wait: Some(retry), tree_hash_hex: Some(tree.id().to_string())})?;

                let remote_url = format!("{}/p/{}/push?{}", config.git_host(), project_id, query);
                let mut push_opts = PushOptions::new();
                push_opts.remote_callbacks(push_cbs);
                let mut remote = repo
                    .remote_anonymous(remote_url.clone().as_str())
                    .context("Failed to create an anonymous remote in the shadow git project")?;

                //  push to non-main branch so that we dont get "branch is currently checked out" error
                //  https://stackoverflow.com/questions/2816369/git-push-error-remote-rejected-master-master-branch-is-currently-checked
                //  TODO: potential race condition as another process could update HEAD before this push
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
            .context("Failed to spawn the tokio runtime")?
            .context("Failed to git push")?)
    }
}

struct RecallGitTransport;

impl git2::transport::SmartSubtransport for RecallGitTransport {
    fn action(
        &self,
        url: &str,
        _action: git2::transport::Service,
    ) -> Result<Box<dyn git2::transport::SmartSubtransportStream>, git2::Error> {
        git_smart_transport_action(url, _action, Scheme::HTTP)
    }

    fn close(&self) -> Result<(), git2::Error> {
        Ok(())
    }
}

struct RecallsGitTransport;
impl git2::transport::SmartSubtransport for RecallsGitTransport {
    fn action(
        &self,
        url: &str,
        _action: git2::transport::Service,
    ) -> Result<Box<dyn git2::transport::SmartSubtransportStream>, git2::Error> {
        git_smart_transport_action(url, _action, Scheme::HTTPS)
    }

    fn close(&self) -> Result<(), git2::Error> {
        Ok(())
    }
}

fn git_smart_transport_action(
    url: &str,
    _action: git2::transport::Service,
    scheme: Scheme,
) -> Result<Box<dyn git2::transport::SmartSubtransportStream>, git2::Error> {
    use git2::{Error, ErrorClass, ErrorCode};

    trace!("creating transport for url {}", url);

    let uri = hyper::Uri::try_from(url)
        .map_err(|e| {
            error!("{}", &e);
            e
        })
        .map_err(|e| Error::new(ErrorCode::Invalid, ErrorClass::Config, e.to_string()))?;

    let mut parts = uri.into_parts();
    parts.scheme = Some(scheme);
    let uri = hyper::Uri::from_parts(parts)
        .map_err(|e| {
            error!("{}", &e);
            e
        })
        .map_err(|e| Error::new(ErrorCode::Invalid, ErrorClass::Config, e.to_string()))?;

    // let runtime = tokio::runtime::Runtime::new().unwrap();
    let handle = tokio::runtime::Handle::current();
    let conn = handle
        .block_on(git_conn(uri))
        .map_err(|e| {
            error!("{}", &e);
            e
        })
        .map_err(|e| Error::new(ErrorCode::GenericError, ErrorClass::Http, e.to_string()))?;

    Ok(Box::new(conn))
}
struct RecallGitConn(hyper::upgrade::Upgraded);

async fn git_conn(url: hyper::Uri) -> Result<RecallGitConn> {
    // TODO: move this into a param
    let access_token = read_global_config(get_global_config_dir()?)?
        .access_token()
        .ok_or(anyhow::anyhow!("no configured access_token"))?;
    //  https://github.com/hyperium/hyper/blob/master/examples/upgrades.rs
    let upgrade_req = hyper::Request::builder()
        .method("POST")
        .uri(url.to_string())
        .header(UPGRADE, "recall-git")
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .body(Body::empty())
        .context(format!("Failed to construct post for {}", url.to_string()))?;

    use hyper_tls::HttpsConnector;
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    let res = client.request(upgrade_req).await.context(format!(
        "Failed to send upgrade request for {}",
        url.to_string()
    ))?;

    if res.status().eq(&StatusCode::UNAUTHORIZED) {
        return Err(anyhow!("Something is wrong with your access token, perhaps you've been logged out by the server? You can login again at https://buildrecall.com/setup".to_string()));
    }

    let conn = hyper::upgrade::on(res)
        .await
        .context(format!("Failed to upgrade: {}", url.to_string()))?;

    Ok(RecallGitConn(conn))
}

impl std::io::Read for RecallGitConn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let handle = tokio::runtime::Handle::current();
        let result = handle.block_on(async { self.0.read(buf).await });
        result
    }
}

impl std::io::Write for RecallGitConn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(self.0.write(buf))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(self.0.flush())
    }
}

#[cfg(test)]
mod test {

    use axum::http::HeaderValue;
    use git2::{PushOptions, RemoteCallbacks};
    use hyper::{header, upgrade::OnUpgrade, StatusCode};

    use super::*;
    #[tokio::test]
    async fn test_git_upgrade() -> Result<()> {
        tracing_subscriber::fmt::init();

        tokio::spawn(TestGitRemote::start());
        trace!("initing git transport");
        init_git_transport();
        trace!("init git transport");

        let handle = tokio::runtime::Handle::current();

        handle
            .spawn_blocking(move || -> Result<_> {
                //  push to non-main branch so that we dont get "branch is currently checked out" error
                //  https://stackoverflow.com/questions/2816369/git-push-error-remote-rejected-master-master-branch-is-currently-checked
                let refspecs: &[&str] = &["+HEAD:refs/heads/incoming"];

                let mut push_cbs = RemoteCallbacks::new();
                push_cbs.push_update_reference(|ref_, msg| {
                    eprintln!("{:?}", (ref_, msg));
                    Ok(())
                });

                let mut push_opts = PushOptions::new();
                push_opts.remote_callbacks(push_cbs);

                let repo = TempGitRepo::init()?;
                trace!("temp git repo ready");
                let mut remote = repo.remote_anonymous("recall+git://localhost:7890/push")?;
                // let mut remote = repo.remote("recall", "recall+git://localhost:7890")?;
                Ok(remote.push(refspecs, Some(&mut push_opts))?)
            })
            .await??;
        trace!("here");

        Ok(())
    }

    struct TempGitRepo {
        path: std::path::PathBuf,
        repo: git2::Repository,
    }

    impl std::ops::Deref for TempGitRepo {
        type Target = git2::Repository;

        fn deref(&self) -> &Self::Target {
            &self.repo
        }
    }

    impl TempGitRepo {
        pub fn init() -> Result<Self> {
            let rand_path: [u8; 16] = rand::random();
            let rand_path =
                std::env::temp_dir().join(base64::encode_config(rand_path, base64::URL_SAFE));
            trace!("creating temp git repo {:?}", rand_path);

            let repo = git2::Repository::init(rand_path.clone())?;
            let sig = git2::Signature::now("test", "test")?;
            {
                let mut index = repo.index()?;
                let tree_id = index.write_tree()?;
                let tree = repo.find_tree(tree_id)?;
                repo.commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    &format!("Initial test commit {:?}", rand_path),
                    &tree,
                    &[],
                )?;
            }

            Ok(Self {
                path: rand_path.clone(),
                repo,
            })
        }
    }

    impl Drop for TempGitRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    struct TestGitRemote {}
    impl TestGitRemote {
        async fn start() {
            let app = axum::Router::new().route("/", axum::handler::post(handle_test_git_conn));

            axum::Server::bind(&"127.0.0.1:7890".parse().unwrap())
                .serve(app.into_make_service())
                .await
                .unwrap();
        }
    }

    async fn handle_test_git_conn(
        mut req: axum::http::Request<axum::body::Body>,
    ) -> axum::http::Response<axum::body::Body> {
        let upgrade = req.extensions_mut().remove::<OnUpgrade>().unwrap();

        tokio::spawn(async move {
            let conn = upgrade.await.unwrap();
            trace!("upgrade done!");
            let (mut rd, mut wr) = tokio::io::split(conn);

            git2::Repository::init("/tmp/gittest");
            let mut child = tokio::process::Command::new("git")
                .args(["receive-pack", "/tmp/gittest"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .unwrap();

            let mut stdin = child.stdin.take().unwrap();
            let mut stdout = child.stdout.take().unwrap();

            tokio::spawn(async move { tokio::io::copy(&mut rd, &mut stdin).await });
            tokio::spawn(async move { tokio::io::copy(&mut stdout, &mut wr).await });

            let status = child.wait().await.unwrap();
            if !status.success() {
                panic!("bad exit code {:?}", status);
            }
        });

        trace!("got request");

        axum::http::Response::builder()
            .status(StatusCode::SWITCHING_PROTOCOLS)
            .header(
                header::CONNECTION,
                HeaderValue::from_str("upgrade").unwrap(),
            )
            .header(
                header::UPGRADE,
                HeaderValue::from_str("recall-git").unwrap(),
            )
            .body(Default::default())
            .unwrap()
    }
}

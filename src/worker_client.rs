use std::{convert::TryFrom, sync::Once};

use crate::{global_config::read_global_config, Result};
use hyper::{
    header::{AUTHORIZATION, UPGRADE},
    http::uri::Scheme,
    Body, Client,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::*;

fn init() -> Result<()> {
    tracing_subscriber::fmt::init();
    init_git_transport();

    Ok(())
}

pub async fn push_to_worker() -> Result<()> {
    init()?;

    let repo_path = ".";

    use git2::{PushOptions, RemoteCallbacks};

    //  push to non-main branch so that we dont get "branch is currently checked out" error
    //  https://stackoverflow.com/questions/2816369/git-push-error-remote-rejected-master-master-branch-is-currently-checked
    let refspecs: &[&str] = &["+HEAD:refs/heads/incoming"];

    Ok(tokio::runtime::Handle::current()
        .spawn_blocking(move || {
            let repo = git2::Repository::open(repo_path)?;
            let mut push_cbs = RemoteCallbacks::new();
            push_cbs.push_update_reference(|ref_, msg| {
                eprintln!("{:?}", (ref_, msg));
                Ok(())
            });

            let mut push_opts = PushOptions::new();
            push_opts.remote_callbacks(push_cbs);
            let mut remote = repo.remote_anonymous("recall+git://localhost:7890/push")?;

            remote.push(refspecs, Some(&mut push_opts))
        })
        .await??)
}

const RECALL_GIT_SCHEME_HTTP: &str = "recall+git";
const RECALL_GIT_SCHEME_HTTPS: &str = "recalls+git";

fn init_git_transport() {
    static INIT: Once = Once::new();

    INIT.call_once(move || unsafe {
        git2::transport::register(RECALL_GIT_SCHEME_HTTP, |remote| {
            git2::transport::Transport::smart(remote, false, RecallGitTransport)
        })
        .unwrap();
    });
}

struct RecallGitTransport;

impl git2::transport::SmartSubtransport for RecallGitTransport {
    fn action(
        &self,
        url: &str,
        _action: git2::transport::Service,
    ) -> Result<Box<dyn git2::transport::SmartSubtransportStream>, git2::Error> {
        use git2::{Error, ErrorClass, ErrorCode};

        trace!("creating transport for url {}", url);

        let uri = hyper::Uri::try_from(url)
            .map_err(|e| Error::new(ErrorCode::Invalid, ErrorClass::Config, e.to_string()))?;

        let mut parts = uri.into_parts();
        parts.scheme = Some(Scheme::HTTP);
        let uri = hyper::Uri::from_parts(parts)
            .map_err(|e| Error::new(ErrorCode::Invalid, ErrorClass::Config, e.to_string()))?;

        // let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = tokio::runtime::Handle::current();
        let conn = handle
            .block_on(git_conn(uri))
            .map_err(|e| Error::new(ErrorCode::GenericError, ErrorClass::Http, e.to_string()))?;

        Ok(Box::new(conn))
    }

    fn close(&self) -> Result<(), git2::Error> {
        Ok(())
    }
}
struct RecallGitConn(hyper::upgrade::Upgraded);

async fn git_conn(url: hyper::Uri) -> Result<RecallGitConn> {
    let access_token = read_global_config()?
        .access_token
        .ok_or(anyhow::anyhow!("no configured access_token"))?;
    //  https://github.com/hyperium/hyper/blob/master/examples/upgrades.rs
    let upgrade_req = hyper::Request::builder()
        .method("POST")
        .uri(url.to_string())
        //TODO: add bearer auth header
        .header(UPGRADE, "recall-git")
        .header(AUTHORIZATION, format!("Bearer: {}", access_token))
        .body(Body::empty())?;

    let res = Client::new().request(upgrade_req).await?;

    let conn = hyper::upgrade::on(res).await?;

    Ok(RecallGitConn(conn))
}

async fn send_latest_commit() {}

fn send_diff() {}

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
            use hyper::upgrade::{OnUpgrade, Upgraded};
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

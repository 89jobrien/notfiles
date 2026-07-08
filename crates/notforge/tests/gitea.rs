use std::sync::{Arc, Mutex};

use notforge::config::RepoSpec;
use notforge::error::NotforgeError;
use notforge::gitea::{
    GiteaHttpApi, GiteaHttpRequest, GiteaHttpResponse, GiteaTransport, HttpMethod,
};
use notforge::ports::{ForgeApi, ForgeAuth};
use serde_json::json;

#[derive(Debug, Clone)]
struct RecordedRequest {
    method: HttpMethod,
    url: String,
    authorization: Option<String>,
    body: Option<serde_json::Value>,
}

#[derive(Clone)]
struct FakeTransport {
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    responses: Arc<Mutex<Vec<Result<GiteaHttpResponse, NotforgeError>>>>,
}

impl FakeTransport {
    fn new(responses: Vec<Result<GiteaHttpResponse, NotforgeError>>) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(responses)),
        }
    }

    fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl GiteaTransport for FakeTransport {
    fn send(&self, request: GiteaHttpRequest) -> Result<GiteaHttpResponse, NotforgeError> {
        self.requests.lock().unwrap().push(RecordedRequest {
            method: request.method,
            url: request.url,
            authorization: request.authorization,
            body: request.body,
        });
        self.responses.lock().unwrap().remove(0)
    }
}

#[test]
fn version_gets_gitea_server_version() {
    let transport = FakeTransport::new(vec![Ok(GiteaHttpResponse {
        status: 200,
        body: json!({ "version": "1.22.4" }),
    })]);
    let api = GiteaHttpApi::with_transport("http://gitea.local:3000/", transport.clone());

    let version = api.version().unwrap();

    assert_eq!(version.version, "1.22.4");
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, HttpMethod::Get);
    assert_eq!(requests[0].url, "http://gitea.local:3000/api/v1/version");
    assert_eq!(requests[0].authorization, None);
    assert_eq!(requests[0].body, None);
}

#[test]
fn repo_maps_found_and_not_found_responses() {
    let transport = FakeTransport::new(vec![
        Ok(GiteaHttpResponse {
            status: 200,
            body: json!({
                "owner": { "login": "joe" },
                "name": "notfiles-config",
                "clone_url": "http://gitea.local:3000/joe/notfiles-config.git",
                "ssh_url": "ssh://git@gitea.local:2222/joe/notfiles-config.git"
            }),
        }),
        Ok(GiteaHttpResponse {
            status: 404,
            body: json!({ "message": "not found" }),
        }),
    ]);
    let api = GiteaHttpApi::with_transport("http://gitea.local:3000", transport.clone());

    let found = api.repo("joe", "notfiles-config").unwrap().unwrap();
    let missing = api.repo("joe", "missing").unwrap();

    assert_eq!(found.owner, "joe");
    assert_eq!(found.name, "notfiles-config");
    assert_eq!(
        found.clone_url,
        "http://gitea.local:3000/joe/notfiles-config.git"
    );
    assert_eq!(
        found.ssh_url,
        "ssh://git@gitea.local:2222/joe/notfiles-config.git"
    );
    assert_eq!(missing, None);

    let requests = transport.requests();
    assert_eq!(
        requests[0].url,
        "http://gitea.local:3000/api/v1/repos/joe/notfiles-config"
    );
    assert_eq!(
        requests[1].url,
        "http://gitea.local:3000/api/v1/repos/joe/missing"
    );
}

#[test]
fn create_repo_posts_json_with_token_auth() {
    let transport = FakeTransport::new(vec![Ok(GiteaHttpResponse {
        status: 201,
        body: json!({
            "owner": { "login": "joe" },
            "name": "notfiles-config",
            "clone_url": "http://gitea.local:3000/joe/notfiles-config.git",
            "ssh_url": "ssh://git@gitea.local:2222/joe/notfiles-config.git"
        }),
    })]);
    let api = GiteaHttpApi::with_transport("http://gitea.local:3000", transport.clone());
    let spec = RepoSpec {
        owner: "joe".to_string(),
        name: "notfiles-config".to_string(),
        private: true,
        description: Some("Personal config payload".to_string()),
        remote_name: "gitea".to_string(),
    };

    let repo = api
        .create_repo(
            &spec,
            &ForgeAuth::Token {
                token: "redacted".to_string(),
            },
        )
        .unwrap();

    assert_eq!(repo.owner, "joe");
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, HttpMethod::Post);
    assert_eq!(requests[0].url, "http://gitea.local:3000/api/v1/user/repos");
    assert_eq!(
        requests[0].authorization,
        Some("token redacted".to_string())
    );
    assert_eq!(
        requests[0].body,
        Some(json!({
            "name": "notfiles-config",
            "private": true,
            "description": "Personal config payload"
        }))
    );
}

#[test]
fn create_repo_posts_json_with_basic_auth() {
    let transport = FakeTransport::new(vec![Ok(GiteaHttpResponse {
        status: 201,
        body: json!({
            "owner": { "login": "joe" },
            "name": "notfiles-config",
            "clone_url": "http://gitea.local:3000/joe/notfiles-config.git",
            "ssh_url": "ssh://git@gitea.local:2222/joe/notfiles-config.git"
        }),
    })]);
    let api = GiteaHttpApi::with_transport("http://gitea.local:3000", transport.clone());
    let spec = RepoSpec {
        owner: "joe".to_string(),
        name: "notfiles-config".to_string(),
        private: true,
        description: None,
        remote_name: "gitea".to_string(),
    };

    api.create_repo(
        &spec,
        &ForgeAuth::Basic {
            username: "joe".to_string(),
            password: "secret".to_string(),
        },
    )
    .unwrap();

    let requests = transport.requests();
    assert_eq!(
        requests[0].authorization,
        Some("Basic am9lOnNlY3JldA==".to_string())
    );
    assert_eq!(
        requests[0].body,
        Some(json!({
            "name": "notfiles-config",
            "private": true
        }))
    );
}

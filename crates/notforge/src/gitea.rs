use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::RepoSpec;
use crate::error::NotforgeError;
use crate::ports::{ForgeApi, ForgeAuth, ForgeVersion, RemoteRepository};

/// HTTP method used by the Gitea transport boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// HTTP GET.
    Get,
    /// HTTP POST.
    Post,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get => formatter.write_str("GET"),
            Self::Post => formatter.write_str("POST"),
        }
    }
}

/// Transport-level request sent by the Gitea adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiteaHttpRequest {
    /// HTTP method.
    pub method: HttpMethod,
    /// Absolute URL to call.
    pub url: String,
    /// Optional Authorization header value.
    pub authorization: Option<String>,
    /// Optional JSON body.
    pub body: Option<Value>,
}

/// Transport-level response returned to the Gitea adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiteaHttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Parsed JSON response body, or `null` for an empty body.
    pub body: Value,
}

/// HTTP transport boundary for Gitea API requests.
pub trait GiteaTransport {
    /// Send a transport-level request.
    fn send(&self, request: GiteaHttpRequest) -> Result<GiteaHttpResponse, NotforgeError>;
}

/// Gitea API adapter that implements [`ForgeApi`].
#[derive(Debug, Clone)]
pub struct GiteaHttpApi<T = UreqGiteaTransport> {
    base_url: String,
    transport: T,
}

impl GiteaHttpApi<UreqGiteaTransport> {
    /// Create a Gitea API adapter backed by the default blocking HTTP transport.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::with_transport(base_url, UreqGiteaTransport::default())
    }
}

impl<T> GiteaHttpApi<T>
where
    T: GiteaTransport,
{
    /// Create a Gitea API adapter with an injected transport.
    pub fn with_transport(base_url: impl Into<String>, transport: T) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            transport,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn get(&self, path: &str) -> Result<GiteaHttpResponse, NotforgeError> {
        self.transport.send(GiteaHttpRequest {
            method: HttpMethod::Get,
            url: self.url(path),
            authorization: None,
            body: None,
        })
    }

    fn post(
        &self,
        path: &str,
        auth: &ForgeAuth,
        body: Value,
    ) -> Result<GiteaHttpResponse, NotforgeError> {
        self.transport.send(GiteaHttpRequest {
            method: HttpMethod::Post,
            url: self.url(path),
            authorization: Some(authorization_header(auth)),
            body: Some(body),
        })
    }
}

impl<T> ForgeApi for GiteaHttpApi<T>
where
    T: GiteaTransport,
{
    fn version(&self) -> Result<ForgeVersion, NotforgeError> {
        let response = self.get("/api/v1/version")?;
        require_status(response.status, &[200], "get Gitea version")?;
        let payload: GiteaVersionResponse = parse_body(response.body, "Gitea version response")?;
        Ok(ForgeVersion {
            version: payload.version,
        })
    }

    fn repo(&self, owner: &str, name: &str) -> Result<Option<RemoteRepository>, NotforgeError> {
        let response = self.get(&format!("/api/v1/repos/{owner}/{name}"))?;
        if response.status == 404 {
            return Ok(None);
        }

        require_status(response.status, &[200], "get Gitea repository")?;
        let payload: GiteaRepositoryResponse =
            parse_body(response.body, "Gitea repository response")?;
        Ok(Some(payload.into_remote_repository()))
    }

    fn create_repo(
        &self,
        spec: &RepoSpec,
        auth: &ForgeAuth,
    ) -> Result<RemoteRepository, NotforgeError> {
        let response = self.post("/api/v1/user/repos", auth, create_repo_body(spec))?;
        require_status(response.status, &[200, 201], "create Gitea repository")?;
        let payload: GiteaRepositoryResponse =
            parse_body(response.body, "Gitea repository response")?;
        Ok(payload.into_remote_repository())
    }
}

/// Blocking HTTP transport implemented with `ureq`.
#[derive(Debug, Clone)]
pub struct UreqGiteaTransport {
    agent: ureq::Agent,
}

impl Default for UreqGiteaTransport {
    fn default() -> Self {
        Self {
            agent: ureq::Agent::new(),
        }
    }
}

impl UreqGiteaTransport {
    /// Create a transport with a custom `ureq` agent.
    pub fn new(agent: ureq::Agent) -> Self {
        Self { agent }
    }
}

impl GiteaTransport for UreqGiteaTransport {
    fn send(&self, request: GiteaHttpRequest) -> Result<GiteaHttpResponse, NotforgeError> {
        let method = request.method;
        let url = request.url.clone();
        let mut call = match method {
            HttpMethod::Get => self.agent.get(&url),
            HttpMethod::Post => self.agent.post(&url),
        }
        .set("Accept", "application/json");

        if let Some(authorization) = request.authorization {
            call = call.set("Authorization", &authorization);
        }

        let result = match method {
            HttpMethod::Get => call.call(),
            HttpMethod::Post => call
                .set("Content-Type", "application/json")
                .send_json(request.body.unwrap_or_else(|| json!({}))),
        };

        match result {
            Ok(response) => response_from_ureq(response),
            Err(ureq::Error::Status(status, response)) => response_from_status(status, response),
            Err(err) => Err(NotforgeError::Http(format!(
                "{method} {url} failed before response: {err}"
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GiteaVersionResponse {
    version: String,
}

#[derive(Debug, Deserialize)]
struct GiteaRepositoryResponse {
    owner: GiteaOwnerResponse,
    name: String,
    clone_url: String,
    ssh_url: String,
}

impl GiteaRepositoryResponse {
    fn into_remote_repository(self) -> RemoteRepository {
        RemoteRepository {
            owner: self.owner.login,
            name: self.name,
            clone_url: self.clone_url,
            ssh_url: self.ssh_url,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GiteaOwnerResponse {
    login: String,
}

#[derive(Debug, Serialize)]
struct CreateRepoRequest<'a> {
    name: &'a str,
    private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
}

fn create_repo_body(spec: &RepoSpec) -> Value {
    serde_json::to_value(CreateRepoRequest {
        name: &spec.name,
        private: spec.private,
        description: spec.description.as_deref(),
    })
    .expect("CreateRepoRequest serialization should not fail")
}

fn authorization_header(auth: &ForgeAuth) -> String {
    match auth {
        ForgeAuth::Token { token } => format!("token {token}"),
        ForgeAuth::Basic { username, password } => {
            format!(
                "Basic {}",
                STANDARD.encode(format!("{username}:{password}"))
            )
        }
    }
}

fn require_status(status: u16, expected: &[u16], action: &str) -> Result<(), NotforgeError> {
    if expected.contains(&status) {
        Ok(())
    } else {
        Err(NotforgeError::Http(format!(
            "{action} returned unexpected status {status}"
        )))
    }
}

fn parse_body<T>(body: Value, context: &str) -> Result<T, NotforgeError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(body)
        .map_err(|err| NotforgeError::Http(format!("invalid {context}: {err}")))
}

fn response_from_status(
    status: u16,
    response: ureq::Response,
) -> Result<GiteaHttpResponse, NotforgeError> {
    response_from_parts(status, response)
}

fn response_from_ureq(response: ureq::Response) -> Result<GiteaHttpResponse, NotforgeError> {
    response_from_parts(response.status(), response)
}

fn response_from_parts(
    status: u16,
    response: ureq::Response,
) -> Result<GiteaHttpResponse, NotforgeError> {
    let body = response
        .into_string()
        .map_err(|err| NotforgeError::Http(format!("reading HTTP response body: {err}")))?;
    let body = if body.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&body)
            .map_err(|err| NotforgeError::Http(format!("parsing HTTP response JSON: {err}")))?
    };

    Ok(GiteaHttpResponse { status, body })
}

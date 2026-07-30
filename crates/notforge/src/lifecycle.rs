use crate::config::{ForgeConfig, GiteaMode};
use crate::error::NotforgeError;
use crate::ports::{ForgeApi, ForgeLifecycle, LifecycleStatus};

/// Lifecycle adapter for an already-running Gitea instance.
///
/// Verifies reachability via [`ForgeApi::version`]. Other [`GiteaMode`]
/// variants are not yet supported and return an error.
pub struct ExistingGiteaLifecycle<'a, A> {
    api: &'a A,
}

impl<'a, A> ExistingGiteaLifecycle<'a, A>
where
    A: ForgeApi,
{
    /// Create a lifecycle adapter backed by the given forge API client.
    pub fn new(api: &'a A) -> Self {
        Self { api }
    }
}

impl<A> ForgeLifecycle for ExistingGiteaLifecycle<'_, A>
where
    A: ForgeApi,
{
    fn ensure_available(&self, config: &ForgeConfig) -> Result<LifecycleStatus, NotforgeError> {
        match &config.gitea.mode {
            GiteaMode::Existing => {
                self.api.version()?;
                Ok(LifecycleStatus::Verified)
            }
            other => Err(NotforgeError::Command(format!(
                "gitea mode {other:?} is not yet supported by notstrap wiring; only Existing is implemented"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ForgeAuthConfig, GiteaConfig};
    use crate::gitea::{GiteaHttpApi, GiteaHttpRequest, GiteaHttpResponse, GiteaTransport};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct FakeTransport {
        responses: Arc<Mutex<Vec<Result<GiteaHttpResponse, NotforgeError>>>>,
    }

    impl FakeTransport {
        fn new(responses: Vec<Result<GiteaHttpResponse, NotforgeError>>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses)),
            }
        }
    }

    impl GiteaTransport for FakeTransport {
        fn send(&self, _request: GiteaHttpRequest) -> Result<GiteaHttpResponse, NotforgeError> {
            self.responses.lock().unwrap().remove(0)
        }
    }

    fn forge_config(mode: GiteaMode) -> ForgeConfig {
        ForgeConfig {
            gitea: GiteaConfig {
                base_url: "http://gitea.local:3000".to_string(),
                owner: "joe".to_string(),
                ssh_host: "gitea.local".to_string(),
                ssh_port: 2222,
                mode,
                auth: ForgeAuthConfig::default(),
            },
            repositories: Vec::new(),
        }
    }

    #[test]
    fn existing_mode_verified_when_api_reachable() {
        let transport = FakeTransport::new(vec![Ok(GiteaHttpResponse {
            status: 200,
            body: json!({ "version": "1.22.4" }),
        })]);
        let api = GiteaHttpApi::with_transport("http://gitea.local:3000", transport);
        let lifecycle = ExistingGiteaLifecycle::new(&api);
        let cfg = forge_config(GiteaMode::Existing);

        assert_eq!(
            lifecycle.ensure_available(&cfg).unwrap(),
            LifecycleStatus::Verified
        );
    }

    #[test]
    fn existing_mode_propagates_api_error() {
        let transport = FakeTransport::new(vec![Err(NotforgeError::Http(
            "connection refused".to_string(),
        ))]);
        let api = GiteaHttpApi::with_transport("http://gitea.local:3000", transport);
        let lifecycle = ExistingGiteaLifecycle::new(&api);
        let cfg = forge_config(GiteaMode::Existing);

        assert!(lifecycle.ensure_available(&cfg).is_err());
    }

    #[test]
    fn non_existing_mode_returns_unsupported() {
        let transport = FakeTransport::new(vec![]);
        let api = GiteaHttpApi::with_transport("http://gitea.local:3000", transport);
        let lifecycle = ExistingGiteaLifecycle::new(&api);
        let cfg = forge_config(GiteaMode::VmSystemd(crate::config::VmSystemdConfig {
            ssh_user: "dev".to_string(),
            ssh_host: "gitea.local".to_string(),
            service_name: "gitea".to_string(),
        }));

        let err = lifecycle.ensure_available(&cfg).unwrap_err();
        assert!(matches!(err, NotforgeError::Command(_)));
    }
}

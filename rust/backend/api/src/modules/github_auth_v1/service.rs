use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, EndpointNotSet, EndpointSet, RedirectUrl, TokenUrl};

use crate::error::{ErrorCode, ErrorResponse};
use crate::services::oauth;
use crate::services::oauth::redirect_uri;

const GITHUB_DEFAULT_OAUTH_BASE: &str = "https://github.com";
const GITHUB_DEFAULT_API_BASE: &str = "https://api.github.com";

pub fn github_oauth_base() -> String {
    oauth::provider_env_base("GITHUB_OAUTH_BASE_URL", GITHUB_DEFAULT_OAUTH_BASE)
}

pub fn github_api_base() -> String {
    oauth::provider_env_base("GITHUB_API_BASE_URL", GITHUB_DEFAULT_API_BASE)
}

pub struct GitHubCredentials {
    pub client_id: String,
    pub client_secret: String,
}

pub fn load_github_credentials() -> Result<GitHubCredentials, ErrorResponse> {
    let client_id = std::env::var("GITHUB_CLIENT_ID").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("GITHUB_CLIENT_ID not configured")
    })?;
    let client_secret = std::env::var("GITHUB_CLIENT_SECRET").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("GITHUB_CLIENT_SECRET not configured")
    })?;

    Ok(GitHubCredentials {
        client_id,
        client_secret,
    })
}

pub type GithubClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub fn get_github_oauth_client() -> Result<GithubClient, ErrorResponse> {
    let credentials = load_github_credentials()?;
    let redirect_url = redirect_uri("GITHUB_REDIRECT_URI", "github")?;

    let auth_url =
        AuthUrl::new(format!("{}/login/oauth/authorize", github_oauth_base())).map_err(|e| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Invalid GitHub auth URL")
                .with_details(e.to_string())
        })?;
    let token_url = TokenUrl::new(format!("{}/login/oauth/access_token", github_oauth_base()))
        .map_err(|e| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Invalid GitHub token URL")
                .with_details(e.to_string())
        })?;

    let client = BasicClient::new(ClientId::new(credentials.client_id))
        .set_client_secret(ClientSecret::new(credentials.client_secret))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|e| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Invalid GitHub redirect URI")
                .with_details(e.to_string())
        })?);

    Ok(client)
}

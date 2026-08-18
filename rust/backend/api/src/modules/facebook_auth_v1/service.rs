use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, EndpointNotSet, EndpointSet, RedirectUrl, TokenUrl};

use crate::error::{ErrorCode, ErrorResponse};

pub const FACEBOOK_GRAPH_API_VERSION: &str = "v26.0";

pub fn facebook_graph_url(path: &str) -> String {
    format!(
        "https://graph.facebook.com/{FACEBOOK_GRAPH_API_VERSION}/{}",
        path.trim_start_matches('/')
    )
}

fn facebook_web_url(path: &str) -> String {
    format!(
        "https://www.facebook.com/{FACEBOOK_GRAPH_API_VERSION}/{}",
        path.trim_start_matches('/')
    )
}

pub struct FacebookCredentials {
    pub client_id: String,
    pub client_secret: String,
}

pub fn load_facebook_credentials() -> Result<FacebookCredentials, ErrorResponse> {
    let client_id = std::env::var("FACEBOOK_CLIENT_ID").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("FACEBOOK_CLIENT_ID not configured")
    })?;
    let client_secret = std::env::var("FACEBOOK_CLIENT_SECRET").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("FACEBOOK_CLIENT_SECRET not configured")
    })?;

    Ok(FacebookCredentials {
        client_id,
        client_secret,
    })
}

pub type FacebookClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub fn get_facebook_oauth_client() -> Result<FacebookClient, ErrorResponse> {
    let credentials = load_facebook_credentials()?;
    let redirect_url = std::env::var("FACEBOOK_REDIRECT_URI").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("FACEBOOK_REDIRECT_URI not configured")
    })?;

    let auth_url = AuthUrl::new(facebook_web_url("dialog/oauth")).map_err(|e| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Invalid Facebook auth URL")
            .with_details(e.to_string())
    })?;
    let token_url = TokenUrl::new(facebook_graph_url("oauth/access_token")).map_err(|e| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Invalid Facebook token URL")
            .with_details(e.to_string())
    })?;

    let client = BasicClient::new(ClientId::new(credentials.client_id))
        .set_client_secret(ClientSecret::new(credentials.client_secret))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|e| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Invalid Facebook redirect URI")
                .with_details(e.to_string())
        })?);

    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_urls_use_current_central_version() {
        let url = facebook_graph_url("me");

        assert_eq!(url, "https://graph.facebook.com/v26.0/me");
    }
}

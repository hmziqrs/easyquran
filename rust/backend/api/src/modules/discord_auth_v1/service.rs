use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, EndpointNotSet, EndpointSet, RedirectUrl, TokenUrl};

use crate::error::{ErrorCode, ErrorResponse};
use crate::services::oauth;

const DISCORD_DEFAULT_OAUTH_BASE: &str = "https://discord.com";
const DISCORD_DEFAULT_API_BASE: &str = "https://discord.com/api";

pub fn discord_oauth_base() -> String {
    oauth::provider_env_base("DISCORD_OAUTH_BASE_URL", DISCORD_DEFAULT_OAUTH_BASE)
}

pub fn discord_api_base() -> String {
    oauth::provider_env_base("DISCORD_API_BASE_URL", DISCORD_DEFAULT_API_BASE)
}

pub struct DiscordCredentials {
    pub client_id: String,
    pub client_secret: String,
}

pub fn load_discord_credentials() -> Result<DiscordCredentials, ErrorResponse> {
    let client_id = std::env::var("DISCORD_CLIENT_ID").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("DISCORD_CLIENT_ID not configured")
    })?;
    let client_secret = std::env::var("DISCORD_CLIENT_SECRET").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("DISCORD_CLIENT_SECRET not configured")
    })?;

    Ok(DiscordCredentials {
        client_id,
        client_secret,
    })
}

pub type DiscordClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub fn get_discord_oauth_client() -> Result<DiscordClient, ErrorResponse> {
    let credentials = load_discord_credentials()?;
    let redirect_url = std::env::var("DISCORD_REDIRECT_URI").map_err(|_| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("DISCORD_REDIRECT_URI not configured")
    })?;

    let auth_url =
        AuthUrl::new(format!("{}/oauth2/authorize", discord_oauth_base())).map_err(|e| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Invalid Discord auth URL")
                .with_details(e.to_string())
        })?;
    let token_url = TokenUrl::new(format!("{}/oauth2/token", discord_api_base())).map_err(|e| {
        ErrorResponse::new(ErrorCode::InternalServerError)
            .with_message("Invalid Discord token URL")
            .with_details(e.to_string())
    })?;

    let client = BasicClient::new(ClientId::new(credentials.client_id))
        .set_client_secret(ClientSecret::new(credentials.client_secret))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(RedirectUrl::new(redirect_url).map_err(|e| {
            ErrorResponse::new(ErrorCode::InternalServerError)
                .with_message("Invalid Discord redirect URI")
                .with_details(e.to_string())
        })?);

    Ok(client)
}

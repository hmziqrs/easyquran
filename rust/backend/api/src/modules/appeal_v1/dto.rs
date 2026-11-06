use serde::Deserialize;

/// No `Debug`: the token is a credential and must not reach a log line.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitAppealPayload {
    pub token: String,
    pub statement: String,
}

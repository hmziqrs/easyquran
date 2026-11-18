//! Typed calls for every endpoint `web/` uses, as methods on [`ApiClient`].
//! Bookmark, preference and reading-session sync are not here: the sync
//! slice builds them on [`super::ApiPost`].
//!
//! Admin-only modules (`analytics/v1/*` dashboards, moderation, users
//! admin) are not called by `web/` and are not here either.

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{
    ApiClient, ApiError, ApiReply, Secret,
    types::{
        AccountDeletion, AnalyticsSummary, DayTotal, DeviceChallenge, DeviceRegistered,
        DeviceRegistration, DeviceView, FreshAuthStatus, IdentitySummary, NativeTokenNonce,
        OAuthProvider, PasskeyChallenge, PasskeyView, ProfileUpdate, ProfileUpdateRequest,
        ProviderToken, ReadingEvent, RegisterRequest, ReminderSchedule, ReportBody, SessionInfo,
        SignInOutcome, TwoFaSetup, UserProfile, pending_totp_token,
    },
};

fn decode<T: DeserializeOwned>(value: Value, what: &str) -> Result<T, ApiError> {
    serde_json::from_value(value).map_err(|e| ApiError::Decode(format!("{what}: {e}")))
}

fn field<T: DeserializeOwned>(body: &Value, key: &str) -> Result<T, ApiError> {
    decode(body.get(key).cloned().unwrap_or(Value::Null), key)
}

fn to_value(body: &impl serde::Serialize) -> Result<Value, ApiError> {
    serde_json::to_value(body).map_err(|e| ApiError::Decode(e.to_string()))
}

/// Result of probing `GET /user/v1/get` (`SessionProbeResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionProbe {
    Authenticated(UserProfile),
    /// 401 or 403: no signed-in session.
    Anonymous,
    /// Offline, 5xx or an unreadable body: change nothing.
    Unknown,
}

impl ApiClient {
    /// A sign-in reply: the outcome, with the CSRF token fetched again when
    /// the server did not say it rotated (web `LoginFlow`).
    async fn sign_in_reply(&self, reply: ApiReply) -> Result<SignInOutcome, ApiError> {
        let outcome = SignInOutcome::decode(&reply.body)?;
        if matches!(outcome, SignInOutcome::SignedIn(_)) && !reply.rotated {
            // Best effort, as on web: the next unsafe call fetches anyway.
            let _ = self.refresh_csrf().await;
        }
        Ok(outcome)
    }

    // --- auth/v1 -------------------------------------------------------------

    /// `POST /auth/v1/register`. Signs in nothing: the caller logs in next.
    pub async fn register(&self, request: &RegisterRequest) -> Result<(), ApiError> {
        self.post("/auth/v1/register", to_value(request)?).await?;
        Ok(())
    }

    /// `POST /auth/v1/log_in`.
    pub async fn log_in(&self, email: &str, password: &Secret) -> Result<SignInOutcome, ApiError> {
        let reply = self
            .post(
                "/auth/v1/log_in",
                json!({ "email": email, "password": password.expose() }),
            )
            .await?;
        self.sign_in_reply(reply).await
    }

    /// `POST /auth/v1/login/totp`: finish a `pending_login`.
    pub async fn login_totp(
        &self,
        totp_token: &Secret,
        code: &Secret,
    ) -> Result<SignInOutcome, ApiError> {
        let reply = self
            .post(
                "/auth/v1/login/totp",
                json!({ "totp_token": totp_token.expose(), "code": code.expose() }),
            )
            .await?;
        self.sign_in_reply(reply).await
    }

    /// `GET /auth/v1/login/totp/pending`: the TOTP token this session is
    /// waiting on, if any (`{"status":"none"}` otherwise).
    pub async fn login_totp_pending(&self) -> Result<Option<Secret>, ApiError> {
        let reply = self.get("/auth/v1/login/totp/pending").await?;
        Ok(pending_totp_token(&reply.body))
    }

    /// `POST /auth/v1/log_out`. The reply clears `x-eq-session`; the local
    /// value and CSRF token are cleared here whatever the reply, since a
    /// 401/403 means the session was already gone.
    pub async fn log_out(&self) -> Result<(), ApiError> {
        let result = self.post_empty("/auth/v1/log_out").await;
        let gone = matches!(&result, Err(ApiError::Http(e)) if matches!(e.status, 401 | 403));
        if result.is_ok() || gone {
            self.clear_session();
            return Ok(());
        }
        result.map(|_| ())
    }

    /// `POST /auth/v1/2fa/setup` (`code` when a fresh-auth step asks).
    pub async fn twofa_setup(&self, code: Option<&Secret>) -> Result<TwoFaSetup, ApiError> {
        let reply = match code {
            Some(code) => {
                self.post("/auth/v1/2fa/setup", json!({ "code": code.expose() }))
                    .await?
            }
            None => self.post_empty("/auth/v1/2fa/setup").await?,
        };
        decode(reply.body, "2fa setup")
    }

    /// `POST /auth/v1/2fa/verify`: the updated account.
    pub async fn twofa_verify(
        &self,
        code: &Secret,
        backup_code: Option<&Secret>,
    ) -> Result<UserProfile, ApiError> {
        let mut body = json!({ "code": code.expose() });
        if let Some(backup) = backup_code {
            body["backup_code"] = Value::from(backup.expose());
        }
        let reply = self.post("/auth/v1/2fa/verify", body).await?;
        UserProfile::decode_or_err(&reply.body)
    }

    /// `POST /auth/v1/2fa/disable` with a TOTP or backup code.
    pub async fn twofa_disable(&self, code: &Secret) -> Result<UserProfile, ApiError> {
        let reply = self
            .post(
                "/auth/v1/2fa/disable",
                json!({ "code": code.expose().trim() }),
            )
            .await?;
        UserProfile::decode_or_err(&reply.body)
    }

    /// `POST /auth/v1/reauth`: `true` when the fresh-auth window is open.
    pub async fn reauth(&self, password: &Secret, code: Option<&Secret>) -> Result<bool, ApiError> {
        let mut body = json!({ "password": password.expose() });
        if let Some(code) = code.filter(|c| !c.is_empty()) {
            body["code"] = Value::from(code.expose());
        }
        let reply = self.post("/auth/v1/reauth", body).await?;
        Ok(reply.body.get("ready").and_then(Value::as_bool) == Some(true))
    }

    /// `GET /auth/v1/reauth/status`.
    pub async fn reauth_status(&self) -> Result<FreshAuthStatus, ApiError> {
        let reply = self.get("/auth/v1/reauth/status").await?;
        decode(reply.body, "reauth status")
    }

    /// `GET /auth/v1/sessions/list`: live sessions, the current flagged.
    pub async fn sessions_list(&self) -> Result<Vec<SessionInfo>, ApiError> {
        let reply = self.get("/auth/v1/sessions/list").await?;
        field(&reply.body, "data")
    }

    /// `DELETE /auth/v1/sessions/terminate/{id}`.
    pub async fn sessions_terminate(&self, id: i64) -> Result<(), ApiError> {
        self.delete(&format!("/auth/v1/sessions/terminate/{id}"))
            .await?;
        Ok(())
    }

    // --- user/v1 -------------------------------------------------------------

    /// `GET /user/v1/get`, as a probe.
    pub async fn probe_user(&self) -> SessionProbe {
        match self.get("/user/v1/get").await {
            Ok(reply) => UserProfile::decode(&reply.body)
                .map_or(SessionProbe::Unknown, SessionProbe::Authenticated),
            Err(ApiError::Http(e)) if matches!(e.status, 401 | 403) => SessionProbe::Anonymous,
            Err(_) => SessionProbe::Unknown,
        }
    }

    /// `GET /user/v1/get`.
    pub async fn get_user(&self) -> Result<UserProfile, ApiError> {
        let reply = self.get("/user/v1/get").await?;
        UserProfile::decode_or_err(&reply.body)
    }

    /// `POST /user/v1/update`.
    pub async fn update_profile(
        &self,
        request: &ProfileUpdateRequest,
    ) -> Result<ProfileUpdate, ApiError> {
        let reply = self.post("/user/v1/update", to_value(request)?).await?;
        Ok(ProfileUpdate {
            user: UserProfile::decode_or_err(&reply.body)?,
            email_change_pending: reply
                .body
                .get("email_change_pending")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    /// `POST /user/v1/delete` (fresh auth required). The server ends this
    /// session; the local value is cleared here.
    pub async fn delete_account(&self, confirm_email: &str) -> Result<AccountDeletion, ApiError> {
        let reply = self
            .post("/user/v1/delete", json!({ "confirmEmail": confirm_email }))
            .await?;
        self.clear_session();
        decode(reply.body, "account deletion")
    }

    /// `POST /user/v1/age/declare` (`YYYY-MM-DD`): the updated account.
    pub async fn declare_age(&self, date_of_birth: &str) -> Result<UserProfile, ApiError> {
        let reply = self
            .post(
                "/user/v1/age/declare",
                json!({ "date_of_birth": date_of_birth }),
            )
            .await?;
        UserProfile::decode_or_err(&reply.body)
    }

    /// `GET /user/v1/identities`.
    pub async fn identities(&self) -> Result<IdentitySummary, ApiError> {
        let reply = self.get("/user/v1/identities").await?;
        decode(reply.body, "identities")
    }

    /// `POST /user/v1/identities/unlink` (fresh auth required).
    pub async fn unlink_identity(&self, provider: &str) -> Result<IdentitySummary, ApiError> {
        let reply = self
            .post(
                "/user/v1/identities/unlink",
                json!({ "provider": provider }),
            )
            .await?;
        decode(reply.body, "identities")
    }

    // --- email_verification/v1, forgot_password/v1 -----------------------------

    /// `POST /email_verification/v1/verify` (8-character code).
    pub async fn verify_email(&self, code: &Secret) -> Result<(), ApiError> {
        self.post(
            "/email_verification/v1/verify",
            json!({ "code": code.expose() }),
        )
        .await?;
        Ok(())
    }

    /// `POST /email_verification/v1/resend`.
    pub async fn resend_verification(&self) -> Result<(), ApiError> {
        self.post_empty("/email_verification/v1/resend").await?;
        Ok(())
    }

    /// `POST /forgot_password/v1/request`.
    pub async fn forgot_password_request(&self, email: &str) -> Result<(), ApiError> {
        self.post("/forgot_password/v1/request", json!({ "email": email }))
            .await?;
        Ok(())
    }

    /// `POST /forgot_password/v1/verify` (6 digits): the reset token.
    pub async fn forgot_password_verify(
        &self,
        email: &str,
        code: &Secret,
    ) -> Result<Secret, ApiError> {
        let reply = self
            .post(
                "/forgot_password/v1/verify",
                json!({ "email": email, "code": code.expose() }),
            )
            .await?;
        field(&reply.body, "reset_token")
    }

    /// `POST /forgot_password/v1/reset`.
    pub async fn forgot_password_reset(
        &self,
        reset_token: &Secret,
        password: &Secret,
        confirm_password: &Secret,
    ) -> Result<(), ApiError> {
        self.post(
            "/forgot_password/v1/reset",
            json!({
                "reset_token": reset_token.expose(),
                "password": password.expose(),
                "confirm_password": confirm_password.expose(),
            }),
        )
        .await?;
        Ok(())
    }

    // --- passkey/v1 ----------------------------------------------------------

    /// `POST /passkey/v1/list`.
    pub async fn passkeys(&self) -> Result<Vec<PasskeyView>, ApiError> {
        let reply = self.post("/passkey/v1/list", json!({})).await?;
        field(&reply.body, "data")
    }

    /// `POST /passkey/v1/login/begin`.
    pub async fn passkey_login_begin(&self) -> Result<PasskeyChallenge, ApiError> {
        let reply = self.post("/passkey/v1/login/begin", json!({})).await?;
        decode(reply.body, "passkey challenge")
    }

    /// `POST /passkey/v1/login/finish` with the authenticator's assertion.
    pub async fn passkey_login_finish(
        &self,
        credential: Value,
        state_handle: &Secret,
    ) -> Result<SignInOutcome, ApiError> {
        let reply = self
            .post(
                "/passkey/v1/login/finish",
                json!({ "credential": credential, "state_handle": state_handle.expose() }),
            )
            .await?;
        self.sign_in_reply(reply).await
    }

    /// `POST /passkey/v1/register/begin`.
    pub async fn passkey_register_begin(&self) -> Result<PasskeyChallenge, ApiError> {
        let reply = self.post("/passkey/v1/register/begin", json!({})).await?;
        decode(reply.body, "passkey challenge")
    }

    /// `POST /passkey/v1/register/finish` with the attestation.
    pub async fn passkey_register_finish(
        &self,
        credential: Value,
        state_handle: &Secret,
        device_type: Option<&str>,
        transports: Option<Value>,
    ) -> Result<PasskeyView, ApiError> {
        let mut body = json!({ "credential": credential, "state_handle": state_handle.expose() });
        if let Some(kind) = device_type {
            body["device_type"] = Value::from(kind);
        }
        if let Some(t) = transports {
            body["transports"] = t;
        }
        let reply = self.post("/passkey/v1/register/finish", body).await?;
        decode(reply.body, "passkey")
    }

    /// `POST /passkey/v1/remove`.
    pub async fn passkey_remove(&self, credential_id: &str) -> Result<(), ApiError> {
        self.post(
            "/passkey/v1/remove",
            json!({ "credential_id": credential_id }),
        )
        .await?;
        Ok(())
    }

    // --- auth/{provider}/v1 --------------------------------------------------

    /// `POST /auth/{google,apple}/v1/token/nonce`.
    pub async fn oauth_token_nonce(
        &self,
        provider: OAuthProvider,
    ) -> Result<NativeTokenNonce, ApiError> {
        if !provider.has_token_nonce() {
            return Err(ApiError::Decode(format!(
                "{} has no token nonce",
                provider.as_str()
            )));
        }
        let reply = self
            .post_empty(&format!("/auth/{}/v1/token/nonce", provider.as_str()))
            .await?;
        decode(reply.body, "token nonce")
    }

    /// `POST /auth/{provider}/v1/token`: sign in with a provider SDK token.
    pub async fn oauth_token(
        &self,
        provider: OAuthProvider,
        token: &ProviderToken,
    ) -> Result<SignInOutcome, ApiError> {
        let reply = self
            .post(
                &format!("/auth/{}/v1/token", provider.as_str()),
                token.body(),
            )
            .await?;
        self.sign_in_reply(reply).await
    }

    // --- device/v1 -----------------------------------------------------------

    /// `POST /device/v1/challenge`.
    pub async fn device_challenge(&self) -> Result<DeviceChallenge, ApiError> {
        let reply = self.post("/device/v1/challenge", json!({})).await?;
        decode(reply.body, "device challenge")
    }

    /// `POST /device/v1/register`.
    pub async fn device_register(
        &self,
        registration: &DeviceRegistration,
    ) -> Result<DeviceRegistered, ApiError> {
        let reply = self
            .post("/device/v1/register", to_value(registration)?)
            .await?;
        decode(reply.body, "device register")
    }

    /// `POST /device/v1/list`.
    pub async fn devices(&self) -> Result<Vec<DeviceView>, ApiError> {
        let reply = self.post("/device/v1/list", json!({})).await?;
        field(&reply.body, "devices")
    }

    /// `POST /device/v1/rename`: `false` when a newer name won.
    pub async fn device_rename(
        &self,
        expected_user_id: i32,
        device_id: &str,
        name: &str,
        name_stamp: &str,
    ) -> Result<bool, ApiError> {
        let reply = self
            .post(
                "/device/v1/rename",
                json!({
                    "expected_user_id": expected_user_id,
                    "device_id": device_id,
                    "name": name,
                    "name_stamp": name_stamp,
                }),
            )
            .await?;
        Ok(reply.body.get("renamed").and_then(Value::as_bool) == Some(true))
    }

    /// `POST /device/v1/delete`: revokes the device and ends its sessions.
    pub async fn device_delete(
        &self,
        expected_user_id: i32,
        device_id: &str,
    ) -> Result<bool, ApiError> {
        let reply = self
            .post(
                "/device/v1/delete",
                json!({ "expected_user_id": expected_user_id, "device_id": device_id }),
            )
            .await?;
        Ok(reply.body.get("revoked").and_then(Value::as_bool) == Some(true))
    }

    /// `POST /device/v1/token/delete`: drop a push token (first, on sign-out).
    pub async fn device_token_delete(
        &self,
        expected_user_id: i32,
        token: &str,
    ) -> Result<bool, ApiError> {
        let reply = self
            .post(
                "/device/v1/token/delete",
                json!({ "expected_user_id": expected_user_id, "token": token }),
            )
            .await?;
        Ok(reply.body.get("removed").and_then(Value::as_bool) == Some(true))
    }

    // --- notification/v1 ------------------------------------------------------

    /// `POST /notification/v1/reminder/get`: `None` when nothing is stored.
    pub async fn reminder_get(&self, kind: &str) -> Result<Option<ReminderSchedule>, ApiError> {
        let reply = self
            .post("/notification/v1/reminder/get", json!({ "kind": kind }))
            .await?;
        field(&reply.body, "reminder")
    }

    /// `POST /notification/v1/reminder/set`: the stored schedule.
    pub async fn reminder_set(
        &self,
        schedule: &ReminderSchedule,
    ) -> Result<ReminderSchedule, ApiError> {
        let reply = self
            .post("/notification/v1/reminder/set", to_value(schedule)?)
            .await?;
        field(&reply.body, "reminder")
    }

    // --- newsletter/v1, report/v1, appeal/v1 -----------------------------------

    /// `POST /newsletter/v1/subscribe`.
    pub async fn newsletter_subscribe(&self, email: &str) -> Result<(), ApiError> {
        self.post("/newsletter/v1/subscribe", json!({ "email": email }))
            .await?;
        Ok(())
    }

    /// `POST /newsletter/v1/confirm` with the link's email and token.
    pub async fn newsletter_confirm(&self, email: &str, token: &Secret) -> Result<(), ApiError> {
        self.post(
            "/newsletter/v1/confirm",
            json!({ "email": email, "token": token.expose() }),
        )
        .await?;
        Ok(())
    }

    /// `POST /report/v1/submit`.
    pub async fn report_submit(&self, body: &ReportBody) -> Result<(), ApiError> {
        self.post("/report/v1/submit", to_value(body)?).await?;
        Ok(())
    }

    /// `POST /appeal/v1/submit`: `true` once received.
    pub async fn appeal_submit(&self, token: &Secret, statement: &str) -> Result<bool, ApiError> {
        let reply = self
            .post(
                "/appeal/v1/submit",
                json!({ "token": token.expose(), "statement": statement }),
            )
            .await?;
        Ok(reply.body.get("received").and_then(Value::as_bool) == Some(true))
    }

    // --- user/analytics/v1 ----------------------------------------------------

    /// `POST /user/analytics/v1/events`: how many were accepted.
    pub async fn analytics_events(
        &self,
        expected_user_id: i32,
        events: &[ReadingEvent],
    ) -> Result<u64, ApiError> {
        let reply = self
            .post(
                "/user/analytics/v1/events",
                json!({ "expectedUserId": expected_user_id, "events": to_value(&events)? }),
            )
            .await?;
        Ok(reply
            .body
            .get("accepted")
            .and_then(Value::as_u64)
            .unwrap_or(0))
    }

    /// `GET /user/analytics/v1/days?from&to` (`YYYY-MM-DD`, inclusive).
    pub async fn analytics_days(&self, from: &str, to: &str) -> Result<Vec<DayTotal>, ApiError> {
        let valid = |d: &str| d.len() == 10 && d.bytes().all(|b| b.is_ascii_digit() || b == b'-');
        if !valid(from) || !valid(to) {
            return Err(ApiError::Decode("day must be YYYY-MM-DD".into()));
        }
        let reply = self
            .get(&format!("/user/analytics/v1/days?from={from}&to={to}"))
            .await?;
        field(&reply.body, "days")
    }

    /// `GET /user/analytics/v1/summary`.
    pub async fn analytics_summary(&self) -> Result<AnalyticsSummary, ApiError> {
        let reply = self.get("/user/analytics/v1/summary").await?;
        decode(reply.body, "analytics summary")
    }
}

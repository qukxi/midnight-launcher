//! Public-client Device Code flow. Secrets never cross the IPC boundary.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

const DEVICE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "XboxLive.signin offline_access";
#[derive(Default)]
pub struct AuthState {
    pub manager: Mutex<AuthManager>,
    pub cancelled: AtomicBool,
}
#[derive(Default)]
pub struct AuthManager {
    pending: Option<Pending>,
    session: Option<Session>,
    diagnostics: Option<AuthDiagnostics>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthDiagnostics {
    client_id: String,
    attempts: Vec<AuthAttempt>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthAttempt {
    endpoint: &'static str,
    http_status: u16,
    error_code: Option<String>,
}
impl AuthDiagnostics {
    fn new(client: &str) -> Self {
        Self {
            client_id: Uuid::parse_str(client)
                .map(|id| id.to_string())
                .unwrap_or_else(|_| "invalid".into()),
            attempts: Vec::new(),
        }
    }
}
// This contains public application metadata and local error codes, never identity responses.
pub fn diagnostics(manager: &AuthManager) -> Option<AuthDiagnostics> {
    manager.diagnostics.clone()
}
struct Pending {
    uri: String,
    id: String,
    client: String,
    code: String,
    expires: Instant,
    next_poll: Instant,
    interval: u64,
}
#[derive(Clone)]
pub struct Session {
    pub profile: Profile,
    pub token: String,
    pub xuid: String,
    pub client: String,
    expires: Instant,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePrompt {
    pub id: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}
#[derive(Serialize)]
pub struct PollResult {
    pub state: String,
    pub profile: Option<Profile>,
    pub interval: u64,
}
#[derive(Serialize)]
pub struct AuthStatus {
    pub configured: bool,
    pub implemented: bool,
    pub message: String,
}
#[derive(Serialize, Deserialize)]
struct Saved {
    client: String,
    refresh: String,
    profile: Option<Profile>,
}
#[derive(Deserialize)]
struct DeviceReply {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default = "five")]
    interval: u64,
}
fn five() -> u64 {
    5
}
fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .user_agent("MidnightLauncher/0.2.0")
        .build()
        .map_err(|_| "AUTH_NETWORK_CLIENT".into())
}
fn credential() -> Result<keyring::Entry, String> {
    if !cfg!(any(target_os = "macos", target_os = "windows")) {
        return Err("KEYCHAIN_UNSUPPORTED_PLATFORM".into());
    }
    keyring::Entry::new("dev.midnight.launcher.microsoft", "active-account")
        .map_err(|_| "KEYCHAIN_UNAVAILABLE".into())
}
fn saved() -> Result<Option<Saved>, String> {
    match credential()?.get_password() {
        Ok(s) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|_| "KEYCHAIN_INVALID_DATA".into()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("KEYCHAIN_READ_FAILED".into()),
    }
}
fn save(value: &Saved) -> Result<(), String> {
    let secret = serde_json::to_string(value).map_err(|_| "KEYCHAIN_SERIALIZE_FAILED")?;
    credential()?
        .set_password(&secret)
        .map_err(|_| "KEYCHAIN_WRITE_FAILED".into())
}
pub fn valid_client_id(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|v| !v.is_nil())
}
pub fn status(id: &str) -> AuthStatus {
    AuthStatus {
        configured: valid_client_id(id.trim()),
        implemented: true,
        message: if valid_client_id(id.trim()) {
            "AUTH_CONFIGURED"
        } else {
            "AUTH_CLIENT_ID_REQUIRED"
        }
        .into(),
    }
}
// Never return response bodies from identity endpoints: they may include tokens.
async fn response(reply: reqwest::Response, step: &str) -> Result<Value, String> {
    let status = reply.status();
    if status.is_redirection() {
        return Err(format!("AUTH_{step}_REDIRECT_REJECTED"));
    }
    if reply.content_length().is_some_and(|n| n > 1024 * 1024) {
        return Err("AUTH_RESPONSE_TOO_LARGE".into());
    }
    let mut reply = reply;
    let mut bytes = Vec::new();
    while let Some(chunk) = reply
        .chunk()
        .await
        .map_err(|_| format!("AUTH_{step}_NETWORK"))?
    {
        if bytes.len() + chunk.len() > 1024 * 1024 {
            return Err("AUTH_RESPONSE_TOO_LARGE".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let parsed = serde_json::from_slice::<Value>(&bytes);
    // Preserve HTTP errors even if a gateway returns HTML or an empty body.
    let value = parsed.as_ref().unwrap_or(&Value::Null);
    if !status.is_success() {
        if step == "MICROSOFT" {
            let code = value["error"].as_str().unwrap_or("unknown");
            let known = match code {
                "authorization_pending"
                | "slow_down"
                | "authorization_declined"
                | "expired_token"
                | "bad_verification_code"
                | "invalid_grant"
                | "invalid_client"
                | "unauthorized_client"
                | "invalid_scope" => code,
                _ => "request_failed",
            };
            return Err(format!("AUTH_MICROSOFT_{known}"));
        }
        if step == "XSTS" {
            return Err(match value["XErr"].as_u64() {
                Some(2148916233) => "AUTH_XBOX_PROFILE_REQUIRED".into(),
                Some(2148916238) => "AUTH_XBOX_FAMILY_REQUIRED".into(),
                Some(n) => format!("AUTH_XSTS_{n}"),
                None => format!("AUTH_XSTS_HTTP_{}", status.as_u16()),
            });
        }
        if step == "MINECRAFT" && status.as_u16() == 403 {
            let registration_rejected = ["error", "errorMessage", "message"].iter().any(|field| {
                value[*field].as_str().is_some_and(|s| {
                    let message = s.trim().trim_end_matches('.');
                    message.eq_ignore_ascii_case("Invalid app registration")
                        || message.eq_ignore_ascii_case("InvalidAppRegistration")
                })
            });
            return Err(if registration_rejected {
                "AUTH_MINECRAFT_APP_APPROVAL_REQUIRED"
            } else {
                "AUTH_MINECRAFT_FORBIDDEN"
            }
            .into());
        }
        if step == "PROFILE" && status.as_u16() == 404 {
            return Err("AUTH_MINECRAFT_PROFILE_REQUIRED".into());
        }
        return Err(format!("AUTH_{step}_HTTP_{}", status.as_u16()));
    }
    parsed.map_err(|_| format!("AUTH_{step}_INVALID_RESPONSE"))
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum MinecraftLoginRoute {
    Launcher,
    Xbox,
}
impl MinecraftLoginRoute {
    fn url(self) -> &'static str {
        match self {
            Self::Launcher => "https://api.minecraftservices.com/launcher/login",
            Self::Xbox => "https://api.minecraftservices.com/authentication/login_with_xbox",
        }
    }
    fn body(self, uhs: &str, token: &str) -> Value {
        let identity = format!("XBL3.0 x={uhs};{token}");
        match self {
            Self::Launcher => json!({"xtoken": identity, "platform": "PC_LAUNCHER"}),
            Self::Xbox => json!({"identityToken": identity}),
        }
    }
}
async fn exchange_minecraft<F, Fut>(
    mut send: F,
    cancelled: &AtomicBool,
    diagnostics: &mut AuthDiagnostics,
) -> Result<Value, String>
where
    F: FnMut(MinecraftLoginRoute) -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    for route in [MinecraftLoginRoute::Launcher, MinecraftLoginRoute::Xbox] {
        check_cancelled(cancelled)?;
        let reply = send(route).await.map_err(|_| "AUTH_MINECRAFT_NETWORK")?;
        let status = reply.status().as_u16();
        let result = response(reply, "MINECRAFT").await;
        diagnostics.attempts.push(AuthAttempt {
            endpoint: route.url(),
            http_status: status,
            error_code: result.as_ref().err().cloned(),
        });
        check_cancelled(cancelled)?;
        // Try the standard Xbox exchange with the same identity when the launcher route
        // is unavailable or gives an unspecified denial. Explicit registration rejection
        // and all other errors stay authoritative; neither route skips ownership checks.
        if route == MinecraftLoginRoute::Launcher
            && (matches!(status, 404 | 405)
                || status == 403
                    && result
                        .as_ref()
                        .err()
                        .is_some_and(|e| e == "AUTH_MINECRAFT_FORBIDDEN"))
        {
            continue;
        }
        return result;
    }
    Err("AUTH_MINECRAFT_FORBIDDEN".into())
}
fn field(value: &Value, name: &str) -> Result<String, String> {
    value[name]
        .as_str()
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("AUTH_MISSING_{name}"))
}
pub async fn begin(manager: &mut AuthManager, client_id: &str) -> Result<DevicePrompt, String> {
    let client_id = client_id.trim();
    if profile(manager)?.is_some() {
        return Err("AUTH_ACCOUNT_ALREADY_SAVED".into());
    }
    if !valid_client_id(client_id) {
        return Err("AUTH_CLIENT_ID_REQUIRED".into());
    }
    manager.diagnostics = None;
    if manager
        .pending
        .as_ref()
        .is_some_and(|p| p.expires > Instant::now())
    {
        return Err("AUTH_ALREADY_PENDING".into());
    }
    let value = response(
        http()?
            .post(DEVICE)
            .form(&[("client_id", client_id), ("scope", SCOPE)])
            .send()
            .await
            .map_err(|_| "AUTH_MICROSOFT_NETWORK")?,
        "MICROSOFT",
    )
    .await?;
    let data: DeviceReply =
        serde_json::from_value(value).map_err(|_| "AUTH_DEVICE_INVALID_RESPONSE")?;
    let uri = reqwest::Url::parse(&data.verification_uri).map_err(|_| "AUTH_DEVICE_INVALID_URL")?;
    if uri.scheme() != "https"
        || !matches!(
            uri.host_str(),
            Some(
                "microsoft.com"
                    | "www.microsoft.com"
                    | "login.microsoftonline.com"
                    | "login.live.com"
            )
        )
        || !uri.username().is_empty()
        || uri.password().is_some()
        || uri.port_or_known_default() != Some(443)
    {
        return Err("AUTH_DEVICE_INVALID_URL".into());
    }
    let id = Uuid::new_v4().to_string();
    let interval = data.interval.max(5);
    let expires = data.expires_in.clamp(1, 1800);
    manager.pending = Some(Pending {
        uri: data.verification_uri.clone(),
        id: id.clone(),
        client: client_id.into(),
        code: data.device_code,
        expires: Instant::now() + Duration::from_secs(expires),
        next_poll: Instant::now() + Duration::from_secs(interval),
        interval,
    });
    Ok(DevicePrompt {
        id,
        user_code: data.user_code,
        verification_uri: data.verification_uri,
        expires_in: expires,
        interval,
    })
}
pub fn cancel(manager: &mut AuthManager) {
    manager.pending = None;
}
pub async fn poll(
    manager: &mut AuthManager,
    id: &str,
    cancelled: &AtomicBool,
) -> Result<PollResult, String> {
    check_cancelled(cancelled)?;
    let pending = manager.pending.as_mut().ok_or("AUTH_NO_PENDING_LOGIN")?;
    if pending.id != id {
        return Err("AUTH_SESSION_MISMATCH".into());
    }
    if pending.expires <= Instant::now() {
        manager.pending = None;
        return Err("AUTH_MICROSOFT_expired_token".into());
    }
    if pending.next_poll > Instant::now() {
        return Ok(PollResult {
            state: "pending".into(),
            profile: None,
            interval: pending.interval,
        });
    }
    pending.next_poll = Instant::now() + Duration::from_secs(pending.interval);
    let client = pending.client.clone();
    let tokens = response(
        http()?
            .post(TOKEN)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client.as_str()),
                ("device_code", pending.code.as_str()),
            ])
            .send()
            .await
            .map_err(|_| "AUTH_MICROSOFT_NETWORK")?,
        "MICROSOFT",
    )
    .await;
    check_cancelled(cancelled)?;
    match tokens {
        Err(e) if e == "AUTH_MICROSOFT_authorization_pending" => Ok(PollResult {
            state: "pending".into(),
            profile: None,
            interval: pending.interval,
        }),
        Err(e) if e == "AUTH_MICROSOFT_slow_down" => {
            pending.interval += 5;
            pending.next_poll = Instant::now() + Duration::from_secs(pending.interval);
            Ok(PollResult {
                state: "pending".into(),
                profile: None,
                interval: pending.interval,
            })
        }
        Err(e) => {
            manager.pending = None;
            Err(e)
        }
        Ok(tokens) => {
            manager.pending = None;
            manager.diagnostics = Some(AuthDiagnostics::new(&client));
            let refresh = field(&tokens, "refresh_token")?;
            // Persist the rotating refresh token before later exchanges, even if they fail.
            save(&Saved {
                client: client.clone(),
                refresh: refresh.clone(),
                profile: None,
            })?;
            let session = match minecraft(
                &tokens,
                &client,
                cancelled,
                manager.diagnostics.as_mut().unwrap(),
            )
            .await
            {
                Ok(session) => session,
                Err(error) => {
                    if cancelled.load(Ordering::Relaxed) {
                        let _ = credential()?.delete_credential();
                    }
                    return Err(error);
                }
            };
            let profile = session.profile.clone();
            save(&Saved {
                client,
                refresh,
                profile: Some(profile.clone()),
            })?;
            manager.session = Some(session);
            Ok(PollResult {
                state: "connected".into(),
                profile: Some(profile),
                interval: 0,
            })
        }
    }
}
async fn minecraft(
    tokens: &Value,
    client: &str,
    cancelled: &AtomicBool,
    diagnostics: &mut AuthDiagnostics,
) -> Result<Session, String> {
    check_cancelled(cancelled)?;
    let http = http()?;
    let access = field(tokens, "access_token")?;
    let xbox=response(http.post("https://user.auth.xboxlive.com/user/authenticate").header("x-xbl-contract-version","1").json(&json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={access}")},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"})).send().await.map_err(|_|"AUTH_XBOX_NETWORK")?,"XBOX").await?;
    check_cancelled(cancelled)?;
    let xbox_token = field(&xbox, "Token")?;
    let xsts=response(http.post("https://xsts.auth.xboxlive.com/xsts/authorize").header("x-xbl-contract-version","1").json(&json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[xbox_token]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"})).send().await.map_err(|_|"AUTH_XSTS_NETWORK")?,"XSTS").await?;
    check_cancelled(cancelled)?;
    let claims = &xsts["DisplayClaims"]["xui"][0];
    let uhs = field(claims, "uhs")?;
    let xuid = claims["xid"].as_str().unwrap_or("").to_string();
    let xsts_token = field(&xsts, "Token")?;
    let mc = exchange_minecraft(
        |route| {
            http.post(route.url())
                .header(reqwest::header::ACCEPT, "application/json")
                .json(&route.body(&uhs, &xsts_token))
                .send()
        },
        cancelled,
        diagnostics,
    )
    .await?;
    check_cancelled(cancelled)?;
    let token = field(&mc, "access_token")?;
    let entitlements = response(
        http.get("https://api.minecraftservices.com/entitlements/license")
            .query(&[("requestId", Uuid::new_v4().to_string())])
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|_| "AUTH_ENTITLEMENTS_NETWORK")?,
        "ENTITLEMENTS",
    )
    .await?;
    check_cancelled(cancelled)?;
    if !can_play(&entitlements) {
        return Err("AUTH_MINECRAFT_LICENSE_REQUIRED".into());
    }
    let profile = response(
        http.get("https://api.minecraftservices.com/minecraft/profile")
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|_| "AUTH_PROFILE_NETWORK")?,
        "PROFILE",
    )
    .await?;
    check_cancelled(cancelled)?;
    let profile = Profile {
        id: field(&profile, "id")?,
        name: field(&profile, "name")?,
    };
    if !valid_java_profile(&profile) {
        return Err("AUTH_PROFILE_INVALID".into());
    }
    let lifetime = mc["expires_in"]
        .as_u64()
        .ok_or("AUTH_MISSING_EXPIRY")?
        .clamp(1, 86400);
    Ok(Session {
        profile,
        token,
        xuid,
        client: client.into(),
        expires: Instant::now() + Duration::from_secs(lifetime),
    })
}
fn can_play(value: &Value) -> bool {
    value["items"].as_array().is_some_and(|v| {
        v.iter()
            .any(|i| matches!(i["name"].as_str(), Some("game_minecraft")))
    })
}
pub fn profile(manager: &AuthManager) -> Result<Option<Profile>, String> {
    if let Some(s) = &manager.session {
        return Ok(Some(s.profile.clone()));
    }
    // Startup inspection must never read Keychain or trigger a system password prompt.
    Ok(None)
}
fn valid_java_profile(profile: &Profile) -> bool {
    Uuid::parse_str(&profile.id).is_ok_and(|id| !id.is_nil())
        && (1..=16).contains(&profile.name.len())
        && profile
            .name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
fn verified_offline_account(manager: &AuthManager, cached: Option<&Saved>) -> Option<Profile> {
    if let Some(session) = &manager.session {
        if valid_client_id(&session.client)
            && !session.token.is_empty()
            && session.token != "0"
            && valid_java_profile(&session.profile)
        {
            return Some(session.profile.clone());
        }
    }
    // Saved.profile is written only after Minecraft ownership and profile checks succeed.
    let cached = cached?;
    if !valid_client_id(&cached.client) || cached.refresh.is_empty() {
        return None;
    }
    cached
        .profile
        .as_ref()
        .filter(|p| valid_java_profile(p))
        .cloned()
}
/// Local proof of an earlier successful Minecraft sign-in; never refreshes tokens or uses the network.
/// Called on account-page access or an explicit offline action, not during startup inspection.
pub fn offline_account(manager: &AuthManager) -> Result<Option<Profile>, String> {
    if let Some(profile) = verified_offline_account(manager, None) {
        return Ok(Some(profile));
    }
    let cached = saved()?;
    Ok(verified_offline_account(manager, cached.as_ref()))
}
pub fn require_offline_access(manager: &AuthManager) -> Result<(), String> {
    if offline_account(manager)?.is_some() {
        Ok(())
    } else {
        Err("OFFLINE_LOGIN_REQUIRED".into())
    }
}
pub async fn session(manager: &mut AuthManager) -> Result<&Session, String> {
    if manager
        .session
        .as_ref()
        .is_some_and(|s| s.expires > Instant::now() + Duration::from_secs(60))
    {
        return Ok(manager.session.as_ref().unwrap());
    }
    let old = saved()?.ok_or("AUTH_LOGIN_REQUIRED")?;
    manager.diagnostics = Some(AuthDiagnostics::new(&old.client));
    let tokens = response(
        http()?
            .post(TOKEN)
            .form(&[
                ("client_id", old.client.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", old.refresh.as_str()),
                ("scope", SCOPE),
            ])
            .send()
            .await
            .map_err(|_| "AUTH_MICROSOFT_NETWORK")?,
        "MICROSOFT",
    )
    .await?;
    let refresh = tokens["refresh_token"]
        .as_str()
        .unwrap_or(&old.refresh)
        .to_string();
    save(&Saved {
        client: old.client.clone(),
        refresh: refresh.clone(),
        profile: old.profile,
    })?;
    let session = minecraft(
        &tokens,
        &old.client,
        &AtomicBool::new(false),
        manager.diagnostics.as_mut().unwrap(),
    )
    .await?;
    save(&Saved {
        client: old.client,
        refresh,
        profile: Some(session.profile.clone()),
    })?;
    manager.session = Some(session);
    Ok(manager.session.as_ref().unwrap())
}
pub fn sign_out(manager: &mut AuthManager) -> Result<(), String> {
    match credential()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(_) => return Err("KEYCHAIN_DELETE_FAILED".into()),
    }
    clear_account(manager);
    Ok(())
}
fn clear_account(manager: &mut AuthManager) {
    manager.session = None;
    manager.pending = None;
    manager.diagnostics = None;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_a_real_nonempty_client_id() {
        assert!(!valid_client_id(""));
        assert!(!valid_client_id("00000000-0000-0000-0000-000000000000"));
        assert!(status("").implemented);
        assert!(!status("").configured);
    }
    #[test]
    fn ownership_is_required() {
        assert!(!can_play(&json!({"items":[]})));
        assert!(!can_play(&json!({"items":[{"name":"unrelated"}]})));
        assert!(can_play(&json!({"items":[{"name":"game_minecraft"}]})));
    }
}

pub fn login_uri(manager: &AuthManager, id: &str) -> Result<String, String> {
    let p = manager.pending.as_ref().ok_or("AUTH_NO_PENDING_LOGIN")?;
    if p.id != id || p.expires <= Instant::now() {
        return Err("AUTH_SESSION_MISMATCH".into());
    }
    Ok(p.uri.clone())
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), String> {
    if cancelled.load(Ordering::Relaxed) {
        Err("AUTH_CANCELLED".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    fn reply(status: u16, body: &str) -> reqwest::Response {
        reqwest::Response::from(
            http::Response::builder()
                .status(status)
                .body(reqwest::Body::from(body.to_owned()))
                .unwrap(),
        )
    }
    fn diagnostic() -> AuthDiagnostics {
        AuthDiagnostics::new("11111111-2222-4333-8444-555555555555")
    }
    #[tokio::test]
    async fn launcher_denial_can_use_the_standard_exchange_without_changing_identity() {
        let mut reports = diagnostic();
        let mut routes = Vec::new();
        let mut replies = std::collections::VecDeque::from([
            reply(
                403,
                r#"{"error":"FORBIDDEN","access_token":"must-not-be-reported"}"#,
            ),
            reply(
                200,
                r#"{"access_token":"private-success-token","expires_in":3600}"#,
            ),
        ]);
        let result = exchange_minecraft(
            |route| {
                routes.push(route);
                std::future::ready(Ok(replies.pop_front().unwrap()))
            },
            &AtomicBool::new(false),
            &mut reports,
        )
        .await
        .unwrap();
        assert_eq!(
            routes,
            [MinecraftLoginRoute::Launcher, MinecraftLoginRoute::Xbox]
        );
        assert_eq!(result["access_token"], "private-success-token");
        let launcher = routes[0].body("same-user", "same-xsts-token");
        let standard = routes[1].body("same-user", "same-xsts-token");
        assert_eq!(launcher["xtoken"], standard["identityToken"]);
        assert_eq!(standard.as_object().unwrap().len(), 1);
        let report = serde_json::to_string(&reports).unwrap();
        assert!(report.contains("403"));
        assert!(report.contains("200"));
        assert!(!report.contains("must-not-be-reported"));
        assert!(!report.contains("private-success-token"));
        assert!(!report.contains("same-xsts-token"));
    }
    #[tokio::test]
    async fn explicit_registration_rejection_is_not_retried_or_inferred_from_generic_403() {
        for (body, expected) in [
            (
                r#"{"errorMessage":"Invalid app registration."}"#,
                "AUTH_MINECRAFT_APP_APPROVAL_REQUIRED",
            ),
            (
                r#"{"message":"Invalid app registration"}"#,
                "AUTH_MINECRAFT_APP_APPROVAL_REQUIRED",
            ),
            (r#"{"error":"FORBIDDEN"}"#, "AUTH_MINECRAFT_FORBIDDEN"),
            (
                "<html>gateway denied request</html>",
                "AUTH_MINECRAFT_FORBIDDEN",
            ),
        ] {
            assert_eq!(
                response(reply(403, body), "MINECRAFT").await.unwrap_err(),
                expected
            );
        }
        let mut count = 0;
        let mut reports = diagnostic();
        let error = exchange_minecraft(
            |_| {
                count += 1;
                std::future::ready(Ok(reply(
                    403,
                    r#"{"errorMessage":"Invalid app registration"}"#,
                )))
            },
            &AtomicBool::new(false),
            &mut reports,
        )
        .await
        .unwrap_err();
        assert_eq!(error, "AUTH_MINECRAFT_APP_APPROVAL_REQUIRED");
        assert_eq!(count, 1);
    }
    #[tokio::test]
    async fn both_denied_routes_still_fail_and_never_create_a_session() {
        let mut reports = diagnostic();
        let error = exchange_minecraft(
            |_| {
                std::future::ready(Ok(reply(
                    403,
                    r#"{"error":"FORBIDDEN","refresh_token":"secret-value"}"#,
                )))
            },
            &AtomicBool::new(false),
            &mut reports,
        )
        .await
        .unwrap_err();
        assert_eq!(error, "AUTH_MINECRAFT_FORBIDDEN");
        assert_eq!(reports.attempts.len(), 2);
        assert!(!serde_json::to_string(&reports)
            .unwrap()
            .contains("secret-value"));
    }
    #[tokio::test]
    async fn credential_errors_server_errors_and_redirects_are_not_retried() {
        for status in [301, 400, 401, 429, 500, 502, 503] {
            let mut count = 0;
            let mut reports = diagnostic();
            assert!(exchange_minecraft(
                |_| {
                    count += 1;
                    std::future::ready(Ok(reply(status, "<html>not JSON</html>")))
                },
                &AtomicBool::new(false),
                &mut reports
            )
            .await
            .is_err());
            assert_eq!(count, 1);
            assert_eq!(reports.attempts[0].http_status, status);
        }
    }
    #[tokio::test]
    async fn cancellation_prevents_the_compatibility_request() {
        let cancelled = AtomicBool::new(false);
        let mut count = 0;
        let mut reports = diagnostic();
        let error = exchange_minecraft(
            |_| {
                count += 1;
                cancelled.store(true, Ordering::Relaxed);
                std::future::ready(Ok(reply(403, "{}")))
            },
            &cancelled,
            &mut reports,
        )
        .await
        .unwrap_err();
        assert_eq!(error, "AUTH_CANCELLED");
        assert_eq!(count, 1);
        let mut reports = diagnostic();
        let error = exchange_minecraft(
            |_| {
                panic!("A cancelled flow must not send a request");
                #[allow(unreachable_code)]
                std::future::ready(Ok(reply(200, "{}")))
            },
            &cancelled,
            &mut reports,
        )
        .await
        .unwrap_err();
        assert_eq!(error, "AUTH_CANCELLED");
        assert!(reports.attempts.is_empty());
    }
    #[tokio::test]
    async fn missing_launcher_route_is_compatible_but_invalid_success_responses_are_not() {
        for status in [404, 405] {
            let mut reports = diagnostic();
            let mut replies =
                std::collections::VecDeque::from([reply(status, ""), reply(200, "{}")]);
            exchange_minecraft(
                |_| std::future::ready(Ok(replies.pop_front().unwrap())),
                &AtomicBool::new(false),
                &mut reports,
            )
            .await
            .unwrap();
            assert_eq!(reports.attempts.len(), 2);
        }
        let mut reports = diagnostic();
        let error = exchange_minecraft(
            |_| std::future::ready(Ok(reply(200, "private-non-json-response"))),
            &AtomicBool::new(false),
            &mut reports,
        )
        .await
        .unwrap_err();
        assert_eq!(error, "AUTH_MINECRAFT_INVALID_RESPONSE");
        assert_eq!(reports.attempts.len(), 1);
        assert!(!serde_json::to_string(&reports)
            .unwrap()
            .contains("private-non-json-response"));
        assert!(diagnostics(&AuthManager::default()).is_none());
    }
    #[tokio::test]
    async fn error_responses_do_not_expose_tokens() {
        let reply = reqwest::Response::from(
            http::Response::builder()
                .status(400)
                .body(reqwest::Body::from(
                    r#"{"error":"invalid_client","access_token":"never-expose-this"}"#,
                ))
                .unwrap(),
        );
        let error = response(reply, "MICROSOFT").await.unwrap_err();
        assert_eq!(error, "AUTH_MICROSOFT_invalid_client");
        assert!(!error.contains("never-expose"));
    }
    #[tokio::test]
    async fn cancelled_poll_never_touches_network_or_keychain() {
        let mut manager = AuthManager::default();
        let result = poll(&mut manager, "unused", &AtomicBool::new(true)).await;
        assert!(matches!(result,Err(e) if e=="AUTH_CANCELLED"));
    }
    #[test]
    fn purchased_product_without_play_entitlement_is_rejected() {
        assert!(!can_play(&json!({"items":[{"name":"product_minecraft"}]})));
    }
}

pub async fn verify_entitlement(session: &Session) -> Result<(), String> {
    let entitlements = response(
        http()?
            .get("https://api.minecraftservices.com/entitlements/license")
            .query(&[("requestId", Uuid::new_v4().to_string())])
            .bearer_auth(&session.token)
            .send()
            .await
            .map_err(|_| "AUTH_ENTITLEMENTS_NETWORK")?,
        "ENTITLEMENTS",
    )
    .await?;
    if !can_play(&entitlements) {
        return Err("AUTH_MINECRAFT_LICENSE_REQUIRED".into());
    }
    Ok(())
}

/// Local identity, never stored as a Microsoft session or sent to identity services.
pub fn launch_offline_identity(
    manager: &AuthManager,
    name: Option<&str>,
) -> Result<Option<Session>, String> {
    guarded_offline_identity(name, || Ok(offline_account(manager)?.is_some()))
}
fn guarded_offline_identity(
    name: Option<&str>,
    has_access: impl FnOnce() -> Result<bool, String>,
) -> Result<Option<Session>, String> {
    let Some(name) = name else { return Ok(None) };
    if !has_access()? {
        return Err("OFFLINE_LOGIN_REQUIRED".into());
    }
    offline_session(name).map(Some)
}
pub fn offline_session(name: &str) -> Result<Session, String> {
    if name.is_empty()
        || name.len() > 16
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err("OFFLINE_INVALID_NAME".into());
    }
    let mut bytes = md5::compute(format!("OfflinePlayer:{name}")).0;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(Session {
        profile: Profile {
            id: Uuid::from_bytes(bytes).simple().to_string(),
            name: name.into(),
        },
        token: "0".into(),
        xuid: String::new(),
        client: String::new(),
        expires: Instant::now(),
    })
}
#[cfg(test)]
mod offline_tests {
    use super::*;
    #[test]
    fn offline_launch_requires_private_account_proof_even_with_a_saved_username() {
        assert_eq!(
            guarded_offline_identity(Some("Player_1"), || Ok(false))
                .err()
                .unwrap(),
            "OFFLINE_LOGIN_REQUIRED"
        );
        assert_eq!(
            guarded_offline_identity(Some("Player_1"), || Ok(true))
                .unwrap()
                .unwrap()
                .profile
                .name,
            "Player_1"
        );
        assert!(guarded_offline_identity(Some("invalid local name"), || Ok(true)).is_err());
        assert_eq!(
            guarded_offline_identity(Some("Player_1"), || Err("KEYCHAIN_READ_FAILED".into()))
                .err()
                .unwrap(),
            "KEYCHAIN_READ_FAILED"
        );
        assert!(guarded_offline_identity(None, || panic!(
            "Online mode must not inspect offline proof"
        ))
        .unwrap()
        .is_none());
    }
    fn verified_saved() -> Saved {
        Saved {
            client: "11111111-2222-4333-8444-555555555555".into(),
            refresh: "synthetic-refresh-for-unit-test".into(),
            profile: Some(Profile {
                id: "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".into(),
                name: "VerifiedPlayer".into(),
            }),
        }
    }
    #[test]
    fn incomplete_sign_in_does_not_unlock_offline_play() {
        let manager = AuthManager::default();
        assert!(verified_offline_account(&manager, None).is_none());
        let mut cached = verified_saved();
        cached.profile = None;
        assert!(verified_offline_account(&manager, Some(&cached)).is_none());
        manager_session_is_not_offline_proof(offline_session("LocalPlayer").unwrap());
    }
    fn manager_session_is_not_offline_proof(session: Session) {
        let manager = AuthManager {
            session: Some(session),
            ..AuthManager::default()
        };
        assert!(verified_offline_account(&manager, None).is_none());
    }
    #[test]
    fn verified_account_survives_restart_without_a_network_refresh() {
        let persisted = serde_json::to_string(&verified_saved()).unwrap();
        let cached: Saved = serde_json::from_str(&persisted).unwrap();
        let manager = AuthManager::default();
        assert_eq!(
            verified_offline_account(&manager, Some(&cached))
                .unwrap()
                .name,
            "VerifiedPlayer"
        );
    }
    #[test]
    fn malformed_cached_account_cannot_grant_offline_access() {
        let manager = AuthManager::default();
        for field in ["client", "refresh", "id", "name"] {
            let mut cached = verified_saved();
            match field {
                "client" => cached.client.clear(),
                "refresh" => cached.refresh.clear(),
                "id" => cached.profile.as_mut().unwrap().id = Uuid::nil().to_string(),
                "name" => cached.profile.as_mut().unwrap().name.clear(),
                _ => unreachable!(),
            }
            assert!(
                verified_offline_account(&manager, Some(&cached)).is_none(),
                "{field}"
            );
        }
    }
    #[test]
    fn signing_out_revokes_an_expired_but_previously_verified_session() {
        let cached = verified_saved();
        let mut manager = AuthManager {
            session: Some(Session {
                profile: cached.profile.unwrap(),
                token: "synthetic-minecraft-token-for-unit-test".into(),
                xuid: String::new(),
                client: cached.client,
                expires: Instant::now() - Duration::from_secs(60),
            }),
            ..AuthManager::default()
        };
        assert!(verified_offline_account(&manager, None).is_some());
        // sign_out deletes the credential first, then calls this same state reset.
        clear_account(&mut manager);
        assert!(verified_offline_account(&manager, None).is_none());
    }
    #[test]
    fn startup_profile_does_not_restore_credentials() {
        assert!(profile(&AuthManager::default()).unwrap().is_none());
    }
    #[test]
    fn java_compatible_identity_and_name_validation() {
        let session = offline_session("Notch").unwrap();
        assert_eq!(session.profile.id, "b50ad385829d3141a2167e7d7539ba7f");
        for name in ["", "bad name", "../world", "abcdefghijklmnopq", "ник"] {
            assert!(offline_session(name).is_err());
        }
    }
}

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
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| format!("AUTH_{step}_INVALID_RESPONSE"))?;
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
            return Err("AUTH_MINECRAFT_APP_APPROVAL_REQUIRED".into());
        }
        if step == "PROFILE" && status.as_u16() == 404 {
            return Err("AUTH_MINECRAFT_PROFILE_REQUIRED".into());
        }
        return Err(format!("AUTH_{step}_HTTP_{}", status.as_u16()));
    }
    Ok(value)
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
            let refresh = field(&tokens, "refresh_token")?;
            // Persist the rotating refresh token before later exchanges, even if they fail.
            save(&Saved {
                client: client.clone(),
                refresh: refresh.clone(),
                profile: None,
            })?;
            let session = match minecraft(&tokens, &client, cancelled).await {
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
    let mc = response(
        http.post("https://api.minecraftservices.com/launcher/login")
            .json(
                &json!({"xtoken":format!("XBL3.0 x={uhs};{xsts_token}"),"platform":"PC_LAUNCHER"}),
            )
            .send()
            .await
            .map_err(|_| "AUTH_MINECRAFT_NETWORK")?,
        "MINECRAFT",
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
    if Uuid::parse_str(&profile.id).is_err()
        || profile.name.len() > 16
        || !profile
            .name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
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
pub async fn session(manager: &mut AuthManager) -> Result<&Session, String> {
    if manager
        .session
        .as_ref()
        .is_some_and(|s| s.expires > Instant::now() + Duration::from_secs(60))
    {
        return Ok(manager.session.as_ref().unwrap());
    }
    let old = saved()?.ok_or("AUTH_LOGIN_REQUIRED")?;
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
    let session = minecraft(&tokens, &old.client, &AtomicBool::new(false)).await?;
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
    manager.session = None;
    manager.pending = None;
    Ok(())
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

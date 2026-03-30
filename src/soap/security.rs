use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha1::{Digest, Sha1};
use std::fmt::Write;

// ── WS-Security Token ──────────────────────────────────────────────────────────

/// ONVIF WS-Security UsernameToken
///
/// PasswordDigest = Base64( SHA-1( Nonce_raw || Created_utf8 || Password_utf8 ) )
///
/// 設計成兩個建構子：
/// - `generate()`：產生隨機 nonce，用於生產環境
/// - `from_parts()`：接受外部值，用於單元測試（可確定性驗證）
#[derive(Debug, Clone)]
pub struct WsSecurityToken {
    pub username: String,
    pub password_digest: String, // Base64(SHA1(nonce||created||password))
    pub nonce_b64: String,       // Base64(nonce_raw)
    pub created: String,         // ISO 8601 UTC, e.g. "2024-01-01T00:00:00Z"
}

impl WsSecurityToken {
    /// 用於測試：傳入已知值，方便驗證輸出
    pub fn from_parts(
        username: impl Into<String>,
        password_digest: impl Into<String>,
        nonce_b64: impl Into<String>,
        created: impl Into<String>,
    ) -> Self {
        Self {
            username: username.into(),
            password_digest: password_digest.into(),
            nonce_b64: nonce_b64.into(),
            created: created.into(),
        }
    }

    /// 用於生產：自動產生隨機 nonce，`device_utc_offset_secs` 是
    /// 裝置 UTC 時間與本機 UTC 時間的差（由 GetSystemDateAndTime 取得）
    pub fn generate(username: &str, password: &str, device_utc_offset_secs: i64) -> Self {
        use rand::RngCore;

        let mut nonce = [0u8; 20];
        rand::thread_rng().fill_bytes(&mut nonce);

        let unix_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            + device_utc_offset_secs;

        let created = unix_secs_to_iso8601(unix_now);
        let digest = compute_digest(&nonce, &created, password);

        Self {
            username: username.to_string(),
            password_digest: STANDARD.encode(digest),
            nonce_b64: STANDARD.encode(nonce),
            created,
        }
    }

    /// 產生 WS-Security XML 片段（不含外層 `<s:Header>` tag）
    pub fn write_xml(&self, out: &mut String) {
        write!(
            out,
            "<wsse:Security>\
               <wsse:UsernameToken>\
                 <wsse:Username>{username}</wsse:Username>\
                 <wsse:Password \
                   Type=\"http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest\"\
                 >{digest}</wsse:Password>\
                 <wsse:Nonce \
                   EncodingType=\"http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary\"\
                 >{nonce}</wsse:Nonce>\
                 <wsu:Created>{created}</wsu:Created>\
               </wsse:UsernameToken>\
             </wsse:Security>",
            username = self.username,
            digest   = self.password_digest,
            nonce    = self.nonce_b64,
            created  = self.created,
        )
        .unwrap();
    }
}

// ── 核心密碼計算（public 供測試直接驗算）──────────────────────────────────────

/// SHA-1( nonce_raw || created_utf8 || password_utf8 ) → 20 bytes
pub fn compute_digest(nonce: &[u8], created: &str, password: &str) -> [u8; 20] {
    let mut h = Sha1::new();
    h.update(nonce);
    h.update(created.as_bytes());
    h.update(password.as_bytes());
    h.finalize().into()
}

// ── UTC 時間格式化（不依賴外部 crate）────────────────────────────────────────

/// Unix timestamp → "YYYY-MM-DDTHH:MM:SSZ"
///
/// 使用 Howard Hinnant 的 civil-from-days 演算法：
/// https://howardhinnant.github.io/date_algorithms.html
pub fn unix_secs_to_iso8601(unix: i64) -> String {
    const SECS_PER_DAY: i64 = 86400;

    let time_of_day = unix.rem_euclid(SECS_PER_DAY);
    let days = (unix - time_of_day) / SECS_PER_DAY;

    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    let z = days + 719_468;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr = if mo <= 2 { y + 1 } else { y };

    format!("{yr:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

// ── 單元測試 ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── 時間格式化 ──────────────────────────────────────────────────────────────

    #[test]
    fn test_epoch_is_1970_01_01() {
        assert_eq!(unix_secs_to_iso8601(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn test_next_day() {
        assert_eq!(unix_secs_to_iso8601(86400), "1970-01-02T00:00:00Z");
    }

    #[test]
    fn test_known_date() {
        // 2024-01-01T00:00:00Z = 1704067200
        assert_eq!(unix_secs_to_iso8601(1_704_067_200), "2024-01-01T00:00:00Z");
    }

    #[test]
    fn test_known_datetime_with_time() {
        // 1718451296 / 86400 = 19889 days remainder 41696 secs
        // 41696s → 11h 34m 56s
        // days=19889 → 2024-06-15
        assert_eq!(unix_secs_to_iso8601(1_718_451_296), "2024-06-15T11:34:56Z");
    }

    // ── digest 計算 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_digest_is_20_bytes() {
        let nonce = [0u8; 20];
        let digest = compute_digest(&nonce, "2024-01-01T00:00:00Z", "password");
        assert_eq!(digest.len(), 20);
    }

    #[test]
    fn test_digest_deterministic() {
        // 相同輸入必須產生相同結果
        let nonce = b"fixed_nonce_for_test";
        let d1 = compute_digest(nonce, "2024-01-01T00:00:00Z", "pass");
        let d2 = compute_digest(nonce, "2024-01-01T00:00:00Z", "pass");
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_different_passwords_give_different_digests() {
        let nonce = b"same_nonce_12345678";
        let d1 = compute_digest(nonce, "2024-01-01T00:00:00Z", "password1");
        let d2 = compute_digest(nonce, "2024-01-01T00:00:00Z", "password2");
        assert_ne!(d1, d2);
    }

    #[test]
    fn test_known_digest_vector() {
        // 測試向量由本函式實際輸出確認後反向固定：
        //   nonce  = [0u8; 20]
        //   created = "2024-01-01T00:00:00Z"
        //   password = "admin"
        //   expected = SHA1(nonce||created||password) → Base64
        //
        // 驗算：用 Python
        //   import hashlib, base64
        //   h = hashlib.sha1(b'\x00'*20 + b'2024-01-01T00:00:00Z' + b'admin').digest()
        //   print(base64.b64encode(h))   →  b'2DXdJ8PbQNGKzH/PeSVx0o7WRHQ='
        let nonce = [0u8; 20];
        let digest = compute_digest(&nonce, "2024-01-01T00:00:00Z", "admin");
        let b64 = STANDARD.encode(digest);
        assert_eq!(b64, "2DXdJ8PbQNGKzH/PeSVx0o7WRHQ=");
    }

    // ── WsSecurityToken XML 輸出 ────────────────────────────────────────────────

    #[test]
    fn test_write_xml_contains_username() {
        let token =
            WsSecurityToken::from_parts("admin", "digest==", "nonce==", "2024-01-01T00:00:00Z");
        let mut out = String::new();
        token.write_xml(&mut out);
        assert!(out.contains("<wsse:Username>admin</wsse:Username>"));
    }

    #[test]
    fn test_write_xml_contains_password_digest() {
        let token = WsSecurityToken::from_parts(
            "admin",
            "digestValue==",
            "nonce==",
            "2024-01-01T00:00:00Z",
        );
        let mut out = String::new();
        token.write_xml(&mut out);
        assert!(out.contains(">digestValue==</wsse:Password>"));
        assert!(out.contains("#PasswordDigest"));
    }

    #[test]
    fn test_write_xml_contains_nonce_and_created() {
        let token = WsSecurityToken::from_parts("u", "d", "nonceB64==", "2024-06-15T12:00:00Z");
        let mut out = String::new();
        token.write_xml(&mut out);
        assert!(out.contains(">nonceB64==</wsse:Nonce>"));
        assert!(out.contains(">2024-06-15T12:00:00Z</wsu:Created>"));
    }

    #[test]
    fn test_write_xml_structure() {
        let token = WsSecurityToken::from_parts("u", "d", "n", "c");
        let mut out = String::new();
        token.write_xml(&mut out);
        assert!(out.starts_with("<wsse:Security>"));
        assert!(out.ends_with("</wsse:Security>"));
        assert!(out.contains("<wsse:UsernameToken>"));
        assert!(out.contains("</wsse:UsernameToken>"));
    }

    #[test]
    fn test_generate_produces_valid_base64_nonce() {
        let token = WsSecurityToken::generate("admin", "password", 0);
        // Base64 字串不應含空白或換行
        assert!(!token.nonce_b64.is_empty());
        assert!(!token.nonce_b64.contains(' '));
        // Base64 decode 後應為 20 bytes
        let decoded = STANDARD.decode(&token.nonce_b64).unwrap();
        assert_eq!(decoded.len(), 20);
    }

    #[test]
    fn test_generate_created_is_iso8601() {
        let token = WsSecurityToken::generate("u", "p", 0);
        // 格式：YYYY-MM-DDTHH:MM:SSZ（固定 20 chars）
        assert_eq!(token.created.len(), 20);
        assert!(token.created.ends_with('Z'));
        assert_eq!(token.created.chars().nth(10), Some('T'));
    }
}

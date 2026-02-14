use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::debug;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

const CODE_LENGTH: usize = 6;
const CODE_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_FAILED_ATTEMPTS: u32 = 5;
const MAX_INVALIDATED_CODES: u32 = 3;
const COOLDOWN_DURATION: Duration = Duration::from_secs(5 * 60);
const HMAC_KEY_SIZE: usize = 32;
const TOKEN_BYTES: usize = 32;
const CODE_CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

const PAIRINGS_FILENAME: &str = "pairings.json";
const KEY_FILENAME: &str = "pairing-key.bin";

#[derive(thiserror::Error, Debug)]
pub enum PairingError {
    #[error("Code expired")]
    NoActiveCode,
    #[error("Code expired")]
    CodeExpired,
    #[error("Invalid code")]
    InvalidCode,
    #[error("Code invalidated")]
    CodeInvalidated,
    #[error("Rate limited")]
    CooldownActive,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedClient {
    pub id: Uuid,
    pub identifier: String,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub last_connected: DateTime<Utc>,
}

struct ActivePairCode {
    code: String,
    created_at: Instant,
    failed_attempts: u32,
}

pub struct PairingManager {
    pairings: Vec<PairedClient>,
    file_path: PathBuf,
    hmac_key: Vec<u8>,
    active_code: Option<ActivePairCode>,
    failed_code_count: u32,
    last_cooldown_start: Option<Instant>,
}

impl PairingManager {
    pub fn new(data_dir: &Path) -> Result<Self, PairingError> {
        let file_path = data_dir.join(PAIRINGS_FILENAME);
        let key_path = data_dir.join(KEY_FILENAME);

        fs::create_dir_all(data_dir)?;

        let pairings = if file_path.exists() {
            let data = fs::read_to_string(&file_path)?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        };

        let hmac_key = if key_path.exists() {
            fs::read(&key_path)?
        } else {
            let key: Vec<u8> = (0..HMAC_KEY_SIZE)
                .map(|_| rand::thread_rng().gen())
                .collect();
            let mut f = fs::File::create(&key_path)?;
            f.write_all(&key)?;
            f.sync_all()?;
            key
        };

        Ok(Self {
            pairings,
            file_path,
            hmac_key,
            active_code: None,
            failed_code_count: 0,
            last_cooldown_start: None,
        })
    }

    pub fn generate_code(&mut self) -> Result<String, PairingError> {
        if self.is_in_cooldown() {
            return Err(PairingError::CooldownActive);
        }

        let mut rng = rand::thread_rng();
        let code: String = (0..CODE_LENGTH)
            .map(|_| {
                let idx = rng.gen_range(0..CODE_CHARSET.len());
                CODE_CHARSET[idx] as char
            })
            .collect();

        self.active_code = Some(ActivePairCode {
            code: code.clone(),
            created_at: Instant::now(),
            failed_attempts: 0,
        });

        debug!("New pair code generated");

        Ok(code)
    }

    /// Whether a non-expired pair code is currently active.
    pub fn has_active_code(&self) -> bool {
        match &self.active_code {
            Some(active) => active.created_at.elapsed() <= CODE_TTL,
            None => false,
        }
    }

    pub fn verify_code(
        &mut self,
        code: &str,
        identifier: &str,
    ) -> Result<(Uuid, String), PairingError> {
        debug!(
            identifier = %identifier,
            has_code = self.active_code.is_some(),
            "Verifying pair code"
        );

        let active = self
            .active_code
            .as_mut()
            .ok_or(PairingError::NoActiveCode)?;

        if active.created_at.elapsed() > CODE_TTL {
            self.active_code = None;
            return Err(PairingError::CodeExpired);
        }

        if !code.eq_ignore_ascii_case(&active.code) {
            active.failed_attempts += 1;
            if active.failed_attempts >= MAX_FAILED_ATTEMPTS {
                self.active_code = None;
                self.failed_code_count += 1;
                if self.failed_code_count >= MAX_INVALIDATED_CODES {
                    self.last_cooldown_start = Some(Instant::now());
                }
                return Err(PairingError::CodeInvalidated);
            }
            return Err(PairingError::InvalidCode);
        }

        // Success - generate token and create pairing
        let token = generate_random_token();
        let token_hash = compute_token_hash(&self.hmac_key, &token);
        let client_id = Uuid::new_v4();
        let now = Utc::now();

        let client = PairedClient {
            id: client_id,
            identifier: identifier.to_string(),
            token_hash,
            created_at: now,
            last_connected: now,
        };

        self.pairings.push(client);
        self.active_code = None;
        self.failed_code_count = 0;
        self.last_cooldown_start = None;
        self.save()?;

        Ok((client_id, token))
    }

    pub fn verify_token(&mut self, token: &str) -> Option<&PairedClient> {
        let hash = compute_token_hash(&self.hmac_key, token);
        let idx = self.pairings.iter().position(|c| c.token_hash == hash)?;
        self.pairings[idx].last_connected = Utc::now();
        // save is best-effort for last_connected update
        let _ = self.save();
        Some(&self.pairings[idx])
    }

    pub fn remove_pairing(&mut self, client_id: Uuid) -> bool {
        let before = self.pairings.len();
        self.pairings.retain(|c| c.id != client_id);
        let removed = self.pairings.len() < before;
        if removed {
            let _ = self.save();
        }
        removed
    }

    pub fn list_pairings(&self) -> &[PairedClient] {
        &self.pairings
    }

    pub fn save(&self) -> Result<(), PairingError> {
        let tmp_path = self.file_path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(&self.pairings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp_path, &self.file_path)?;

        Ok(())
    }

    fn is_in_cooldown(&self) -> bool {
        match self.last_cooldown_start {
            Some(start) => start.elapsed() < COOLDOWN_DURATION,
            None => false,
        }
    }
}

fn compute_token_hash(key: &[u8], token: &str) -> String {
    // HMAC-SHA256 accepts keys of any size, so new_from_slice cannot fail
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC-SHA256 accepts any key size");
    mac.update(token.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn generate_random_token() -> String {
    let bytes: Vec<u8> = (0..TOKEN_BYTES).map(|_| rand::thread_rng().gen()).collect();
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_manager(dir: &Path) -> PairingManager {
        PairingManager::new(dir).unwrap()
    }

    #[test]
    fn test_code_generation_format() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code = mgr.generate_code().unwrap();
        assert_eq!(code.len(), CODE_LENGTH);
        assert!(code
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }

    #[test]
    fn test_code_generation_produces_different_codes() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let mut codes = std::collections::HashSet::new();
        for _ in 0..10 {
            codes.insert(mgr.generate_code().unwrap());
        }
        // Extremely unlikely to get all identical codes
        assert!(codes.len() > 1);
    }

    #[test]
    fn test_verify_correct_code() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code = mgr.generate_code().unwrap();
        let result = mgr.verify_code(&code, "Chrome - Work");
        assert!(result.is_ok());

        let (id, token) = result.unwrap();
        assert!(!token.is_empty());
        assert_eq!(token.len(), TOKEN_BYTES * 2); // hex encoded
        assert_eq!(mgr.list_pairings().len(), 1);
        assert_eq!(mgr.list_pairings()[0].id, id);
        assert_eq!(mgr.list_pairings()[0].identifier, "Chrome - Work");
    }

    #[test]
    fn test_verify_code_case_insensitive() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code = mgr.generate_code().unwrap();
        let lower = code.to_lowercase();
        let result = mgr.verify_code(&lower, "Firefox");
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_wrong_code_fails() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let _code = mgr.generate_code().unwrap();
        let result = mgr.verify_code("ZZZZZZ", "Chrome");
        assert!(matches!(result, Err(PairingError::InvalidCode)));
    }

    #[test]
    fn test_verify_no_active_code() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let result = mgr.verify_code("ABC123", "Chrome");
        assert!(matches!(result, Err(PairingError::NoActiveCode)));
    }

    #[test]
    fn test_code_expiry() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code = mgr.generate_code().unwrap();

        // Manually expire the code by backdating created_at
        if let Some(ref mut active) = mgr.active_code {
            active.created_at = Instant::now() - CODE_TTL - Duration::from_secs(1);
        }

        let result = mgr.verify_code(&code, "Chrome");
        assert!(matches!(result, Err(PairingError::CodeExpired)));
        assert!(mgr.active_code.is_none());
    }

    #[test]
    fn test_five_failures_invalidate_code() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let _code = mgr.generate_code().unwrap();

        for i in 0..4 {
            let result = mgr.verify_code("WRONG1", "Chrome");
            assert!(
                matches!(result, Err(PairingError::InvalidCode)),
                "Attempt {} should be InvalidCode",
                i + 1
            );
        }

        // 5th failure should invalidate the code
        let result = mgr.verify_code("WRONG1", "Chrome");
        assert!(matches!(result, Err(PairingError::CodeInvalidated)));
        assert!(mgr.active_code.is_none());
    }

    #[test]
    fn test_three_invalidated_codes_trigger_cooldown() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        // Invalidate 3 codes
        for _ in 0..MAX_INVALIDATED_CODES {
            let _code = mgr.generate_code().unwrap();
            for _ in 0..MAX_FAILED_ATTEMPTS {
                let _ = mgr.verify_code("WRONG1", "Chrome");
            }
        }

        // Next generate_code should fail with cooldown
        let result = mgr.generate_code();
        assert!(matches!(result, Err(PairingError::CooldownActive)));
    }

    #[test]
    fn test_cooldown_expires() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        // Trigger cooldown
        for _ in 0..MAX_INVALIDATED_CODES {
            let _code = mgr.generate_code().unwrap();
            for _ in 0..MAX_FAILED_ATTEMPTS {
                let _ = mgr.verify_code("WRONG1", "Chrome");
            }
        }

        assert!(matches!(
            mgr.generate_code(),
            Err(PairingError::CooldownActive)
        ));

        // Backdate cooldown start to simulate time passing
        mgr.last_cooldown_start = Some(Instant::now() - COOLDOWN_DURATION - Duration::from_secs(1));

        let result = mgr.generate_code();
        assert!(result.is_ok());
    }

    #[test]
    fn test_successful_pairing_resets_failed_code_count() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        // Invalidate 2 codes
        for _ in 0..2 {
            let _code = mgr.generate_code().unwrap();
            for _ in 0..MAX_FAILED_ATTEMPTS {
                let _ = mgr.verify_code("WRONG1", "Chrome");
            }
        }
        assert_eq!(mgr.failed_code_count, 2);

        // Successful pairing should reset the counter
        let code = mgr.generate_code().unwrap();
        let _ = mgr.verify_code(&code, "Chrome").unwrap();
        assert_eq!(mgr.failed_code_count, 0);
    }

    #[test]
    fn test_token_verification() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code = mgr.generate_code().unwrap();
        let (client_id, token) = mgr.verify_code(&code, "Chrome - Work").unwrap();

        let client = mgr.verify_token(&token);
        assert!(client.is_some());
        let client = client.unwrap();
        assert_eq!(client.id, client_id);
        assert_eq!(client.identifier, "Chrome - Work");
    }

    #[test]
    fn test_invalid_token_verification() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code = mgr.generate_code().unwrap();
        let _ = mgr.verify_code(&code, "Chrome").unwrap();

        let result = mgr.verify_token("totally_invalid_token");
        assert!(result.is_none());
    }

    #[test]
    fn test_persistence_round_trip() {
        let dir = TempDir::new().unwrap();
        let token;
        let client_id;

        {
            let mut mgr = create_manager(dir.path());
            let code = mgr.generate_code().unwrap();
            let (id, tok) = mgr.verify_code(&code, "Firefox - Home").unwrap();
            client_id = id;
            token = tok;
        }

        // Load fresh from disk
        let mut mgr2 = create_manager(dir.path());
        assert_eq!(mgr2.list_pairings().len(), 1);
        assert_eq!(mgr2.list_pairings()[0].id, client_id);
        assert_eq!(mgr2.list_pairings()[0].identifier, "Firefox - Home");

        // Token still works
        let client = mgr2.verify_token(&token);
        assert!(client.is_some());
        assert_eq!(client.unwrap().id, client_id);
    }

    #[test]
    fn test_hmac_key_persisted() {
        let dir = TempDir::new().unwrap();

        let key1 = {
            let mgr = create_manager(dir.path());
            mgr.hmac_key.clone()
        };

        let key2 = {
            let mgr = create_manager(dir.path());
            mgr.hmac_key.clone()
        };

        assert_eq!(key1, key2);
        assert_eq!(key1.len(), HMAC_KEY_SIZE);
    }

    #[test]
    fn test_remove_pairing() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code = mgr.generate_code().unwrap();
        let (id, token) = mgr.verify_code(&code, "Chrome").unwrap();

        assert!(mgr.remove_pairing(id));
        assert!(mgr.list_pairings().is_empty());

        // Token should no longer work
        assert!(mgr.verify_token(&token).is_none());
    }

    #[test]
    fn test_remove_nonexistent_pairing() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        assert!(!mgr.remove_pairing(Uuid::new_v4()));
    }

    #[test]
    fn test_remove_pairing_persisted() {
        let dir = TempDir::new().unwrap();

        let id = {
            let mut mgr = create_manager(dir.path());
            let code = mgr.generate_code().unwrap();
            let (id, _) = mgr.verify_code(&code, "Chrome").unwrap();
            mgr.remove_pairing(id);
            id
        };

        let mgr = create_manager(dir.path());
        assert!(mgr.list_pairings().is_empty());
        assert!(mgr.list_pairings().iter().all(|c| c.id != id));
    }

    #[test]
    fn test_multiple_pairings_independent_verification() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code1 = mgr.generate_code().unwrap();
        let (id1, token1) = mgr.verify_code(&code1, "Chrome - Work").unwrap();

        let code2 = mgr.generate_code().unwrap();
        let (id2, token2) = mgr.verify_code(&code2, "Firefox - Home").unwrap();

        let code3 = mgr.generate_code().unwrap();
        let (id3, token3) = mgr.verify_code(&code3, "Safari - Phone").unwrap();

        assert_eq!(mgr.list_pairings().len(), 3);

        // Each token resolves to the correct client
        let c1 = mgr.verify_token(&token1).unwrap();
        assert_eq!(c1.id, id1);
        assert_eq!(c1.identifier, "Chrome - Work");

        let c2 = mgr.verify_token(&token2).unwrap();
        assert_eq!(c2.id, id2);
        assert_eq!(c2.identifier, "Firefox - Home");

        let c3 = mgr.verify_token(&token3).unwrap();
        assert_eq!(c3.id, id3);
        assert_eq!(c3.identifier, "Safari - Phone");

        // Removing one doesn't affect others
        mgr.remove_pairing(id2);
        assert_eq!(mgr.list_pairings().len(), 2);
        assert!(mgr.verify_token(&token2).is_none());
        assert!(mgr.verify_token(&token1).is_some());
        assert!(mgr.verify_token(&token3).is_some());
    }

    #[test]
    fn test_compute_token_hash_deterministic() {
        let key = b"test-key-123";
        let token = "my-token";

        let hash1 = compute_token_hash(key, token);
        let hash2 = compute_token_hash(key, token);
        assert_eq!(hash1, hash2);

        // Different token produces different hash
        let hash3 = compute_token_hash(key, "other-token");
        assert_ne!(hash1, hash3);

        // Different key produces different hash
        let hash4 = compute_token_hash(b"other-key", token);
        assert_ne!(hash1, hash4);
    }

    #[test]
    fn test_generate_random_token_format() {
        let token = generate_random_token();
        assert_eq!(token.len(), TOKEN_BYTES * 2); // hex encoded
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_code_cleared_after_successful_verification() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code = mgr.generate_code().unwrap();
        let _ = mgr.verify_code(&code, "Chrome").unwrap();

        // Code consumed - second attempt should fail
        let result = mgr.verify_code(&code, "Firefox");
        assert!(matches!(result, Err(PairingError::NoActiveCode)));
    }

    #[test]
    fn test_new_code_replaces_old() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code1 = mgr.generate_code().unwrap();
        let code2 = mgr.generate_code().unwrap();

        // Old code should not work
        if code1 != code2 {
            let result = mgr.verify_code(&code1, "Chrome");
            assert!(matches!(
                result,
                Err(PairingError::InvalidCode) | Err(PairingError::CodeInvalidated)
            ));
        }
    }

    #[test]
    fn test_has_active_code() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        assert!(!mgr.has_active_code());

        let _code = mgr.generate_code().unwrap();
        assert!(mgr.has_active_code());

        // Expire it
        if let Some(ref mut active) = mgr.active_code {
            active.created_at = Instant::now() - CODE_TTL - Duration::from_secs(1);
        }
        assert!(!mgr.has_active_code());
    }

    #[test]
    fn test_has_active_code_after_successful_verify() {
        let dir = TempDir::new().unwrap();
        let mut mgr = create_manager(dir.path());

        let code = mgr.generate_code().unwrap();
        assert!(mgr.has_active_code());

        let _ = mgr.verify_code(&code, "Chrome").unwrap();
        assert!(!mgr.has_active_code());
    }
}

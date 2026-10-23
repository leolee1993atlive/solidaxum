use crate::{
    error::AppResult,
    util::crypto_util::{decrypt_password, encrypt_password},
};

pub trait FieldEncryptor: Send + Sync {
    fn encrypt(&self, plaintext: &str) -> AppResult<String>;
    fn decrypt(&self, ciphertext: &str) -> AppResult<String>;
}

pub struct AesFieldEncryptor {
    key: [u8; 32],
    prefix: String,
}

impl AesFieldEncryptor {
    pub fn new(key: [u8; 32], prefix: impl Into<String>) -> Self {
        Self {
            key,
            prefix: prefix.into(),
        }
    }
}

impl FieldEncryptor for AesFieldEncryptor {
    fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        let encrypted = encrypt_password(&self.key, plaintext)?;
        Ok(format!("{}{}", self.prefix, encrypted))
    }

    fn decrypt(&self, ciphertext: &str) -> AppResult<String> {
        if !ciphertext.starts_with(&self.prefix) {
            return Ok(ciphertext.to_string());
        }
        let stripped = &ciphertext[self.prefix.len()..];
        decrypt_password(&self.key, stripped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes_encrypt_decrypt_round_trip() {
        let key = [0u8; 32];
        let encryptor = AesFieldEncryptor::new(key, "ENC:");
        let plaintext = "sensitive_data_123";
        let ciphertext = encryptor.encrypt(plaintext).unwrap();
        assert!(ciphertext.starts_with("ENC:"));
        assert_ne!(ciphertext, format!("ENC:{}", plaintext));

        let decrypted = encryptor.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_unprefixed_returns_as_is() {
        let key = [0u8; 32];
        let encryptor = AesFieldEncryptor::new(key, "ENC:");
        let plaintext = "plain_value";
        let decrypted = encryptor.decrypt(plaintext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let key = [0u8; 32];
        let wrong_key = [1u8; 32];
        let encryptor = AesFieldEncryptor::new(key, "ENC:");
        let other_encryptor = AesFieldEncryptor::new(wrong_key, "ENC:");
        let ciphertext = other_encryptor.encrypt("data").unwrap();

        let result = encryptor.decrypt(&ciphertext);
        assert!(result.is_err());
    }
}

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine as _, engine::general_purpose};

use crate::error::{AppError, AppResult};

const NONCE_SIZE: usize = 12;

pub fn hash_password(password: &str) -> AppResult<String> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| {
        tracing::error!(error = %e, "密码哈希失败");
        AppError::Internal(format!("密码哈希失败: {}", e))
    })
}

pub fn encrypt_password(key: &[u8], password: &str) -> AppResult<String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AppError::Internal(format!("无效密钥长度: {}", e)))?;

    let nonce_bytes: [u8; NONCE_SIZE] = rand::random();
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext =
        cipher.encrypt(&nonce, password.as_bytes()).map_err(|e| {
            tracing::error!(error = %e, "AES-GCM 加密失败");
            AppError::Internal(format!("加密失败: {}", e))
        })?;

    let mut combined = Vec::new();
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(general_purpose::URL_SAFE_NO_PAD.encode(&combined))
}

pub fn decrypt_password(
    key: &[u8],
    encrypted_password: &str,
) -> AppResult<String> {
    let combined = general_purpose::URL_SAFE_NO_PAD
        .decode(encrypted_password)
        .map_err(|e| {
            tracing::error!(error = %e, "Base64 解码失败");
            AppError::Internal(format!("Base64解码失败: {}", e))
        })?;

    if combined.len() < NONCE_SIZE {
        tracing::error!(
            data_len = combined.len(),
            min_len = NONCE_SIZE,
            "加密数据长度不足"
        );
        return Err(AppError::Internal("无效的加密数据".to_string()));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AppError::Internal(format!("无效密钥长度: {}", e)))?;
    let nonce_arr: [u8; 12] = nonce_bytes
        .try_into()
        .map_err(|_| AppError::Internal("无效的nonce长度".to_string()))?;
    let nonce = Nonce::from(nonce_arr);

    let plaintext = cipher.decrypt(&nonce, ciphertext).map_err(|e| {
        tracing::error!(error = %e, "AES-GCM 解密失败");
        AppError::Internal(format!("解密失败: {}", e))
    })?;

    String::from_utf8(plaintext).map_err(|e| {
        tracing::error!(error = %e, "UTF-8 转换失败");
        AppError::Internal(format!("UTF-8转换失败: {}", e))
    })
}

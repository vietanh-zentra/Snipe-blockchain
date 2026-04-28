use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::Rng;
use sha2::{Digest, Sha256};

type CryptoResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn derive_key_from_password(password: &str) -> [u8; 32] {
    let digest = Sha256::digest(password.as_bytes());
    let mut key = [0_u8; 32];
    key.copy_from_slice(&digest);
    key
}

pub fn encrypt_private_key(plaintext: &str, password: &str) -> CryptoResult<String> {
    let key = derive_key_from_password(password);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| "failed to initialize AES cipher")?;

    let mut nonce_bytes = [0_u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);

    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| "encryption failed")?;

    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(base64::encode(output))
}

pub fn decrypt_private_key(ciphertext_b64: &str, password: &str) -> CryptoResult<String> {
    let combined = base64::decode(ciphertext_b64).map_err(|_| "invalid base64 ciphertext")?;
    if combined.len() < 13 {
        return Err("ciphertext too short".into());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce_arr: [u8; 12] = nonce_bytes
        .try_into()
        .map_err(|_| "invalid nonce length in ciphertext")?;
    let key = derive_key_from_password(password);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| "failed to initialize AES cipher")?;
    let nonce = Nonce::from(nonce_arr);
    let plaintext = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| "decryption failed (wrong password or corrupted data)")?;
    String::from_utf8(plaintext).map_err(|_| "decrypted data is not valid UTF-8".into())
}

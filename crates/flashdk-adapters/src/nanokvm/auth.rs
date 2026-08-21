//! NanoKVM login: replicate the web client's CryptoJS-compatible AES password
//! encryption so we can authenticate and obtain a JWT.
//!
//! The scheme (a hardcoded passphrase + OpenSSL "Salted__" envelope) is both wire-
//! observable and publicly documented (disclosed Feb 2025). We reproduce the
//! *observable transformation* with RustCrypto — no vendor code is used. Note this is
//! obfuscation, not real security: it rides over plaintext HTTP unless the device is
//! configured for TLS.

use aes::cipher::{block_padding::Pkcs7, generic_array::GenericArray, BlockEncryptMut, KeyIvInit};
use aes::Aes256;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use flashdk_core::{Error, Result};
use md5::{Digest, Md5};
use rand::RngCore;

/// The passphrase baked into NanoKVM's frontend bundle (public knowledge).
const PASSPHRASE: &[u8] = b"nanokvm-sipeed-2024";

/// OpenSSL's `EVP_BytesToKey` with MD5 and one iteration, as CryptoJS uses it:
/// derive a 32-byte key and 16-byte IV from passphrase + salt.
fn evp_bytes_to_key(passphrase: &[u8], salt: &[u8]) -> ([u8; 32], [u8; 16]) {
    let mut derived = Vec::new();
    let mut prev: Vec<u8> = Vec::new();
    while derived.len() < 48 {
        let mut h = Md5::new();
        h.update(&prev);
        h.update(passphrase);
        h.update(salt);
        prev = h.finalize().to_vec();
        derived.extend_from_slice(&prev);
    }
    let mut key = [0u8; 32];
    let mut iv = [0u8; 16];
    key.copy_from_slice(&derived[0..32]);
    iv.copy_from_slice(&derived[32..48]);
    (key, iv)
}

/// Encrypt a password the way the NanoKVM web client does, returning the
/// URL-encoded value to place in the login request's `password` field.
pub fn encrypt_password(password: &str) -> String {
    let mut salt = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut salt);
    let (key, iv) = evp_bytes_to_key(PASSPHRASE, &salt);

    let cipher = cbc::Encryptor::<Aes256>::new(
        GenericArray::from_slice(&key),
        GenericArray::from_slice(&iv),
    );
    let ciphertext = cipher.encrypt_padded_vec_mut::<Pkcs7>(password.as_bytes());

    // OpenSSL envelope: "Salted__" + salt + ciphertext, base64-encoded.
    let mut blob = Vec::with_capacity(16 + ciphertext.len());
    blob.extend_from_slice(b"Salted__");
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&ciphertext);
    let b64 = STANDARD.encode(&blob);

    // encodeURIComponent over a base64 string only needs these three characters escaped.
    b64.replace('+', "%2B")
        .replace('/', "%2F")
        .replace('=', "%3D")
}

#[derive(serde::Deserialize)]
struct LoginResp {
    code: i64,
    #[serde(default)]
    data: Option<LoginData>,
}
#[derive(serde::Deserialize)]
struct LoginData {
    token: String,
}

/// Authenticate and return the JWT.
pub async fn login(
    http: &reqwest::Client,
    base_url: &str,
    username: &str,
    password: &str,
) -> Result<String> {
    let body = serde_json::json!({
        "username": username,
        "password": encrypt_password(password),
    });
    let resp = http
        .post(format!("{base_url}/api/auth/login"))
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    let parsed: LoginResp = resp
        .json()
        .await
        .map_err(|e| Error::Protocol(e.to_string()))?;
    match parsed.data {
        Some(d) if parsed.code == 0 => Ok(d.token),
        _ => Err(Error::Auth("invalid username or password".into())),
    }
}

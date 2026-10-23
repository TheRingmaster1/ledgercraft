//! Password protection for company files.
//!
//! A protected file is encrypted with XChaCha20-Poly1305 using a 256-bit key
//! derived from the password with Argon2id. The authenticated encryption means
//! a wrong password, or any change to the file, is detected rather than read
//! as garbage. The salt, nonce and key-derivation settings are stored in the
//! file's header (they are not secret); the password and key never are.
//!
//! The file on disk is always encrypted: saves write the encrypted bytes to a
//! temporary file and rename it over the old one, so no plain copy is ever
//! written. While a company is open its books are, necessarily, decrypted in
//! the app's memory; the key is wiped from memory when the company closes.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::FileError;

pub const CIPHER: &str = "xchacha20poly1305";
pub const KDF: &str = "argon2id";
/// Shortest password accepted for a new company file.
pub const MIN_PASSWORD_CHARS: usize = 12;

/// Argon2id cost settings. The defaults follow OWASP's current guidance
/// (19 MiB of memory, 2 passes, 1 lane): fast enough to open a file in well
/// under a second, slow enough to make guessing passwords expensive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_kib: 19 * 1024,
            iterations: 2,
            parallelism: 1,
        }
    }
}

impl KdfParams {
    /// Settings read from a file are untrusted: refuse ones that would take
    /// absurd memory or time rather than hang or exhaust the machine.
    fn check(&self) -> Result<(), FileError> {
        let ok = (8..=1024 * 1024).contains(&self.memory_kib)
            && (1..=16).contains(&self.iterations)
            && (1..=16).contains(&self.parallelism);
        if ok {
            Ok(())
        } else {
            Err(FileError::Damaged(
                "its encryption settings are out of range".into(),
            ))
        }
    }
}

/// The encryption header stored in a protected file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Header {
    pub(crate) cipher: String,
    pub(crate) kdf: String,
    pub(crate) params: KdfParams,
    pub(crate) salt: String,
    pub(crate) nonce: String,
}

/// The key for one company file, kept while it is open so saving does not
/// ask for the password again. Wiped from memory when dropped.
#[derive(Clone)]
pub struct Protection {
    salt: [u8; 16],
    params: KdfParams,
    key: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for Protection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.write_str("Protection { .. }")
    }
}

impl Protection {
    /// Protection for a new password, with a fresh random salt.
    pub fn new(password: &str) -> Result<Self, FileError> {
        check_new_password(password).map_err(FileError::WeakPassword)?;
        let mut salt = [0u8; 16];
        getrandom::getrandom(&mut salt).map_err(|e| FileError::Crypto(e.to_string()))?;
        let params = KdfParams::default();
        let key = derive(password, &salt, params)?;
        Ok(Self { salt, params, key })
    }

    /// Encrypts `plain` into a header and ciphertext, with a fresh nonce.
    pub(crate) fn seal(&self, plain: &[u8]) -> Result<(Header, String), FileError> {
        let mut nonce = [0u8; 24];
        getrandom::getrandom(&mut nonce).map_err(|e| FileError::Crypto(e.to_string()))?;
        let header = Header {
            cipher: CIPHER.into(),
            kdf: KDF.into(),
            params: self.params,
            salt: B64.encode(self.salt),
            nonce: B64.encode(nonce),
        };
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&self.key[..]));
        let aad = aad(&header);
        let sealed = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plain,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| FileError::Crypto("encryption failed".into()))?;
        Ok((header, B64.encode(sealed)))
    }

    /// Decrypts a protected file's contents with `password`. Returns the
    /// plain text and the protection to save it with again.
    pub(crate) fn open(
        header: &Header,
        ciphertext: &str,
        password: &str,
    ) -> Result<(Zeroizing<Vec<u8>>, Self), FileError> {
        if header.cipher != CIPHER || header.kdf != KDF {
            return Err(FileError::Damaged(format!(
                "it uses an encryption method this version does not know ({} / {})",
                header.cipher, header.kdf
            )));
        }
        header.params.check()?;
        let salt: [u8; 16] = decode_exact(&header.salt)?;
        let nonce: [u8; 24] = decode_exact(&header.nonce)?;
        let sealed = B64
            .decode(ciphertext.trim())
            .map_err(|_| FileError::Damaged("its encrypted data is not readable".into()))?;
        let key = derive(password, &salt, header.params)?;
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&key[..]));
        let aad = aad(header);
        let plain = cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &sealed,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| FileError::WrongPassword)?;
        Ok((
            Zeroizing::new(plain),
            Self {
                salt,
                params: header.params,
                key,
            },
        ))
    }
}

/// Why a new password is not accepted, if it isn't.
pub fn check_new_password(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(format!("Use at least {MIN_PASSWORD_CHARS} characters."));
    }
    if password.trim().is_empty() {
        return Err("The password cannot be only spaces.".into());
    }
    Ok(())
}

/// A one-way check value for a password that guards something inside the
/// books (the closing date): `argon2id$memory$passes$lanes$salt$hash`. The
/// password cannot be read back from it.
pub fn password_check(password: &str) -> Result<String, FileError> {
    if password.trim().is_empty() {
        return Err(FileError::WeakPassword("Enter a password.".into()));
    }
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).map_err(|e| FileError::Crypto(e.to_string()))?;
    let p = KdfParams::default();
    let key = derive(password, &salt, p)?;
    Ok(format!(
        "{KDF}${}${}${}${}${}",
        p.memory_kib,
        p.iterations,
        p.parallelism,
        B64.encode(salt),
        B64.encode(&key[..])
    ))
}

/// Whether `password` is the one a [`password_check`] value was made from.
pub fn password_matches(password: &str, check: &str) -> bool {
    let parts: Vec<&str> = check.split('$').collect();
    let [kdf, memory, passes, lanes, salt, hash] = parts.as_slice() else {
        return false;
    };
    if *kdf != KDF {
        return false;
    }
    let (Ok(memory_kib), Ok(iterations), Ok(parallelism)) =
        (memory.parse(), passes.parse(), lanes.parse())
    else {
        return false;
    };
    let params = KdfParams {
        memory_kib,
        iterations,
        parallelism,
    };
    if params.check().is_err() {
        return false;
    }
    let (Ok(salt), Ok(hash)) = (B64.decode(salt), B64.decode(hash)) else {
        return false;
    };
    match derive(password, &salt, params) {
        // Compare every byte, so the time taken says nothing about how
        // close a guess was.
        Ok(key) => {
            key.len() == hash.len()
                && key
                    .iter()
                    .zip(&hash)
                    .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                    == 0
        }
        Err(_) => false,
    }
}

fn derive(password: &str, salt: &[u8], p: KdfParams) -> Result<Zeroizing<[u8; 32]>, FileError> {
    let params = Params::new(p.memory_kib, p.iterations, p.parallelism, Some(32))
        .map_err(|e| FileError::Crypto(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(password.as_bytes(), salt, &mut key[..])
        .map_err(|e| FileError::Crypto(e.to_string()))?;
    Ok(key)
}

/// Authenticated (but not encrypted) data: binds the header to the
/// ciphertext, so its settings cannot be swapped without detection.
fn aad(h: &Header) -> String {
    format!(
        "ledgercraft:2:{}:{}:{}:{}:{}:{}",
        h.cipher, h.kdf, h.params.memory_kib, h.params.iterations, h.params.parallelism, h.salt
    )
}

fn decode_exact<const N: usize>(text: &str) -> Result<[u8; N], FileError> {
    let bytes = B64
        .decode(text.trim())
        .map_err(|_| FileError::Damaged("its encryption header is not readable".into()))?;
    <[u8; N]>::try_from(bytes.as_slice())
        .map_err(|_| FileError::Damaged("its encryption header has the wrong size".into()))
}

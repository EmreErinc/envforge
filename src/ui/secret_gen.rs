use rand::RngExt;

#[derive(Debug, Clone, PartialEq)]
pub enum SecretGenFormat {
    AlphaNumericSpecial,
    AlphaNumericOnly,
    Hex,
    Base64,
    UuidV4,
}

#[derive(Debug, Clone)]
pub struct SecretGenOpts {
    pub format: SecretGenFormat,
    pub length: usize,
}

impl Default for SecretGenOpts {
    fn default() -> Self {
        Self {
            format: SecretGenFormat::AlphaNumericSpecial,
            length: 32,
        }
    }
}

pub fn generate_secret(opts: &SecretGenOpts) -> String {
    let mut rng = rand::rng();
    match opts.format {
        SecretGenFormat::UuidV4 => uuid::Uuid::new_v4().to_string(),
        SecretGenFormat::Hex => {
            let bytes: Vec<u8> = (0..(opts.length / 2).max(1))
                .map(|_| rng.random())
                .collect();
            hex::encode(bytes)
        }
        SecretGenFormat::Base64 => {
            use base64::Engine;
            let bytes: Vec<u8> = (0..(opts.length * 3 / 4).max(1))
                .map(|_| rng.random())
                .collect();
            base64::engine::general_purpose::STANDARD.encode(bytes)
        }
        SecretGenFormat::AlphaNumericOnly => {
            const CHARSET: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            (0..opts.length)
                .map(|_| {
                    let idx = rng.random_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect()
        }
        SecretGenFormat::AlphaNumericSpecial => {
            const CHARSET: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}";
            (0..opts.length)
                .map(|_| {
                    let idx = rng.random_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect()
        }
    }
}

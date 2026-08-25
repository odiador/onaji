use onaji_protocol::messages::security::AuthCredentials;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct WorkerIdentity {
    pub id: String,
    pub credentials: AuthCredentials,
    pub authenticated: bool,
}

#[derive(Debug, Default)]
pub struct Authenticator {
    valid_tokens: HashSet<String>,
    valid_cert_fingerprints: HashSet<String>,
}

impl Authenticator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_token(&mut self, token: impl Into<String>) {
        self.valid_tokens.insert(token.into());
    }

    pub fn revoke_token(&mut self, token: &str) {
        self.valid_tokens.remove(token);
    }

    pub fn register_cert_fingerprint(&mut self, fingerprint: impl Into<String>) {
        self.valid_cert_fingerprints.insert(fingerprint.into());
    }

    pub fn authenticate(&self, credentials: &AuthCredentials) -> bool {
        match credentials {
            AuthCredentials::Token(token) => self.valid_tokens.contains(token),
            AuthCredentials::Certificate { fingerprint, .. } => {
                self.valid_cert_fingerprints.contains(fingerprint)
            }
        }
    }
}

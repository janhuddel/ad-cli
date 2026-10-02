use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Not logged in. Run '{login_command}' first.")]
    NotLoggedIn { login_command: String },

    #[error("Login failed: {0}")]
    BindFailed(String),

    /// `kind` is the singular object kind, e.g. "user" or "group".
    #[error("No {kind} found matching '{term}'")]
    NotFound { kind: &'static str, term: String },

    /// `kinds` is the plural object kind, e.g. "users" or "groups".
    #[error("'{term}' matches several {kinds}, specify one of:\n{candidates}")]
    Ambiguous {
        kinds: &'static str,
        term: String,
        candidates: String,
    },

    #[error("LDAP error: {0}")]
    Ldap(#[from] ldap3::LdapError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Credential store error: {0}")]
    Credential(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Maps the AD-specific sub-code embedded in a bind failure's diagnostic
/// message (e.g. "80090308: LdapErr: ... data 532, v3839") to a human
/// readable explanation. Falls back to the raw message when no known code
/// is found, which is always the case for non-AD servers.
pub fn explain_bind_failure(diagnostic_message: &str) -> String {
    let code = diagnostic_message
        .split("data ")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_hexdigit()).next())
        .map(|s| s.to_ascii_lowercase());

    let explanation = match code.as_deref() {
        Some("525") => Some("No such user"),
        Some("52e") => Some("Invalid credentials"),
        Some("530") => Some("Not permitted to logon at this time"),
        Some("531") => Some("Not permitted to logon from this workstation"),
        Some("532") => Some("Password expired"),
        Some("533") => Some("Account disabled"),
        Some("701") => Some("Account expired"),
        Some("773") => Some("User must reset password"),
        Some("775") => Some("Account locked out"),
        _ => None,
    };

    match explanation {
        Some(text) => format!("{text} (AD diagnostic: {diagnostic_message})"),
        None => diagnostic_message.to_string(),
    }
}

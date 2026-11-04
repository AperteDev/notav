//! One error type for everything the bindings return.

/// Why a call failed. `reason` is in English, for logs and error details; front-ends word the
/// common cases themselves.
#[derive(Debug, uniffi::Error)]
pub enum NotavError {
    /// The server refused the login name or app password.
    Auth,
    /// Changed on the server meanwhile.
    Conflict,
    NoAccount,
    NotFound {
        reason: String,
    },
    /// A note, folder or account has that name already.
    Exists {
        reason: String,
    },
    /// Not a name a note or folder can have.
    Invalid {
        reason: String,
    },
    /// Locked on the server, by an editor that has it open.
    Locked {
        reason: String,
    },
    Http {
        code: u16,
        reason: String,
    },
    Net {
        reason: String,
    },
    Parse {
        reason: String,
    },
    Db {
        reason: String,
    },
    /// The data was left by a newer Notav: update to open it.
    Newer {
        reason: String,
    },
    /// The account is gone (removed meanwhile).
    Gone,
    /// The keyring did not take the password.
    SavePassword {
        reason: String,
    },
    /// Edits not on the server yet could not go up first; they would be lost.
    Upload {
        reason: String,
    },
    /// The account has edits not on the server yet and is not connected to upload them.
    NotUploaded,
}

impl std::fmt::Display for NotavError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NotavError::Auth => write!(f, "authentication failed"),
            NotavError::Conflict => write!(f, "precondition failed (changed on server)"),
            NotavError::NoAccount => write!(f, "no account configured"),
            NotavError::NotFound { reason } => write!(f, "not found: {reason}"),
            NotavError::Exists { reason } => write!(f, "exists already: {reason}"),
            NotavError::Invalid { reason } => write!(f, "invalid name: {reason}"),
            NotavError::Locked { reason } => write!(f, "locked on the server: {reason}"),
            NotavError::Http { code, reason } => write!(f, "HTTP {code}: {reason}"),
            NotavError::Net { reason } => write!(f, "network: {reason}"),
            NotavError::Parse { reason } => write!(f, "parse: {reason}"),
            NotavError::Db { reason } => write!(f, "database: {reason}"),
            NotavError::Newer { reason } => write!(f, "made by a newer version of Notav: {reason}"),
            NotavError::Gone => write!(f, "the account is gone"),
            NotavError::SavePassword { reason } => write!(f, "cannot save password: {reason}"),
            NotavError::Upload { reason } => write!(f, "cannot upload pending changes: {reason}"),
            NotavError::NotUploaded => write!(f, "changes not uploaded yet"),
        }
    }
}

impl From<notav_core::accounts::Error> for NotavError {
    fn from(e: notav_core::accounts::Error) -> Self {
        use notav_core::accounts::Error as E;
        match e {
            E::Exists => NotavError::Exists {
                reason: "account".into(),
            },
            E::Gone => NotavError::Gone,
            E::SavePassword(reason) => NotavError::SavePassword { reason },
            E::Upload(e) => NotavError::Upload {
                reason: e.to_string(),
            },
            E::NotUploaded => NotavError::NotUploaded,
            E::Core(e) => e.into(),
        }
    }
}

impl std::error::Error for NotavError {}

impl From<notav_core::Error> for NotavError {
    fn from(e: notav_core::Error) -> Self {
        use notav_core::Error as E;
        let shown = e.to_string();
        match e {
            E::Auth => NotavError::Auth,
            E::Conflict => NotavError::Conflict,
            E::NoAccount => NotavError::NoAccount,
            E::NotFound(reason) => NotavError::NotFound { reason },
            E::Exists(reason) => NotavError::Exists { reason },
            E::Invalid(reason) => NotavError::Invalid { reason },
            E::Locked(reason) => NotavError::Locked { reason },
            // The server's message as the core shows it, without "HTTP <code>: ".
            E::Http(code, _) => NotavError::Http {
                code,
                reason: shown
                    .split_once(": ")
                    .map_or(shown.clone(), |(_, m)| m.into()),
            },
            E::Net(reason) => NotavError::Net { reason },
            E::Parse(reason) => NotavError::Parse { reason },
            E::Db(reason) => NotavError::Db { reason },
            E::Newer(reason) => NotavError::Newer { reason },
        }
    }
}

pub type Result<T> = std::result::Result<T, NotavError>;

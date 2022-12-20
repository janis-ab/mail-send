/*
 * Copyright Stalwart Labs Ltd.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

//! # mail-send
//!
//! [![crates.io](https://img.shields.io/crates/v/mail-send)](https://crates.io/crates/mail-send)
//! [![build](https://github.com/stalwartlabs/mail-send/actions/workflows/rust.yml/badge.svg)](https://github.com/stalwartlabs/mail-send/actions/workflows/rust.yml)
//! [![docs.rs](https://img.shields.io/docsrs/mail-send)](https://docs.rs/mail-send)
//! [![crates.io](https://img.shields.io/crates/l/mail-send)](http://www.apache.org/licenses/LICENSE-2.0)
//!
//! _mail-send_ is a Rust library to build, sign and send e-mail messages via SMTP. It includes the following features:
//!
//! - Generates **e-mail** messages conforming to the Internet Message Format standard (_RFC 5322_).
//! - Full **MIME** support (_RFC 2045 - 2049_) with automatic selection of the most optimal encoding for each message body part.
//! - DomainKeys Identified Mail (**DKIM**) Signatures (_RFC 6376_) with ED25519-SHA256, RSA-SHA256 and RSA-SHA1 support.
//! - Simple Mail Transfer Protocol (**SMTP**; _RFC 5321_) delivery.
//! - SMTP Service Extension for Secure SMTP over **TLS** (_RFC 3207_).
//! - SMTP Service Extension for Authentication (_RFC 4954_) with automatic mechanism negotiation (from most secure to least secure):
//!   - CRAM-MD5 (_RFC 2195_)
//!   - DIGEST-MD5 (_RFC 2831_; obsolete but still supported)
//!   - XOAUTH2 (Google proprietary)
//!   - LOGIN
//!   - PLAIN
//! - Full async (requires Tokio).
//!
//! ## Usage Example
//!
//! Send a message via an SMTP server that requires authentication:
//!
//! ```rust
//!     // Build a simple multipart message
//!     let message = MessageBuilder::new()
//!         .from(("John Doe", "john@example.com"))
//!         .to(vec![
//!             ("Jane Doe", "jane@example.com"),
//!             ("James Smith", "james@test.com"),
//!         ])
//!         .subject("Hi!")
//!         .html_body("<h1>Hello, world!</h1>")
//!         .text_body("Hello world!");
//!
//!     // Connect to the SMTP submissions port, upgrade to TLS and
//!     // authenticate using the provided credentials.
//!     SmtpClientBuilder::new()
//!         .connect_starttls("smtp.gmail.com", 587)
//!         .await
//!         .unwrap()
//!         .authenticate(("john", "p4ssw0rd"))
//!         .await
//!         .unwrap()
//!         .send(message)
//!         .await
//!         .unwrap();
//! ```
//!
//! Sign a message with DKIM and send it via an SMTP relay server:
//!
//! ```rust
//!     // Build a simple text message with a single attachment
//!     let message = MessageBuilder::new()
//!         .from(("John Doe", "john@example.com"))
//!         .to("jane@example.com")
//!         .subject("Howdy!")
//!         .text_body("These pretzels are making me thirsty.")
//!         .binary_attachment("image/png", "pretzels.png", [1, 2, 3, 4].as_ref());
//!
//!     // Sign an e-mail message using RSA-SHA256
//!     let pk_rsa = RsaKey::<Sha256>::from_pkcs1_pem(TEST_KEY).unwrap();
//!     let signature_rsa = Signature::new()
//!         .headers(["From", "To", "Subject"])
//!         .domain("example.com")
//!         .selector("default")
//!         .expiration(60 * 60 * 7); // Number of seconds before this signature expires (optional)
//!
//!     // Connect to an SMTP relay server over TLS and
//!     // sign the message with the provided DKIM signature.
//!     SmtpClientBuilder::new()
//!         .connect_tls("smtp.example.com", 465)
//!         .await
//!         .unwrap()
//!         .send_signed(message, &pk_rsa, signature_rsa)
//!         .await
//!         .unwrap();
//! ```
//!
//! More examples of how to build messages are available in the [`mail-builder`](https://crates.io/crates/mail-builder) crate.
//! Please note that this library does not support parsing e-mail messages as this functionality is provided separately by the [`mail-parser`](https://crates.io/crates/mail-parser) crate.
//!
//! ## Testing
//!
//! To run the testsuite:
//!
//! ```bash
//!  $ cargo test --all-features
//! ```
//!
//! or, to run the testsuite with MIRI:
//!
//! ```bash
//!  $ cargo +nightly miri test --all-features
//! ```
//!
//! ## License
//!
//! Licensed under either of
//!
//!  * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
//!  * MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
//!
//! at your option.
//!
//! ## Copyright
//!
//! Copyright (C) 2020-2022, Stalwart Labs Ltd.
//!
//! See [COPYING] for the license.
//!
//! [COPYING]: https://github.com/stalwartlabs/mail-send/blob/main/COPYING
//!

pub mod smtp;
use std::{borrow::Cow, fmt::Display, time::Duration};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector;

#[cfg(feature = "builder")]
pub use mail_builder;

#[cfg(feature = "dkim")]
pub use mail_auth;

#[derive(Debug)]
pub enum Error {
    /// I/O error
    Io(std::io::Error),

    /// Base64 decode error
    Base64(base64::DecodeError),

    // SMTP authentication error.
    Auth(smtp::auth::Error),

    /// Failure parsing SMTP reply
    UnparseableReply,

    /// Unexpected SMTP reply.
    UnexpectedReply(smtp_proto::Response<String>),

    /// SMTP authentication failure.
    AuthenticationFailed(smtp_proto::Response<String>),

    /// Invalid TLS name provided.
    InvalidTLSName,

    /// Missing authentication credentials.
    MissingCredentials,

    /// Missing message sender.
    MissingMailFrom,

    /// Missing message recipients.
    MissingRcptTo,

    /// The server does no support any of the available authentication methods.
    UnsupportedAuthMechanism,

    /// Connection timeout.
    Timeout,

    /// STARTTLS not available
    MissingStartTls,
}

pub type Result<T> = std::result::Result<T, Error>;

/// SMTP client builder
#[derive(Clone)]
pub struct SmtpClientBuilder {
    pub timeout: Duration,
    pub tls: TlsConnector,
}

/// SMTP client builder
pub struct SmtpClient<T: AsyncRead + AsyncWrite, U> {
    stream: T,
    timeout: Duration,
    capabilities: U,
}

#[derive(Clone)]
pub struct Credentials<'x> {
    pub username: Cow<'x, str>,
    pub secret: Cow<'x, str>,
}

pub struct Connected;
pub struct Disconnected;

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::Base64(e) => write!(f, "Base64 decode error: {}", e),
            Error::Auth(e) => write!(f, "SMTP authentication error: {}", e),
            Error::UnparseableReply => write!(f, "Unparseable SMTP reply"),
            Error::UnexpectedReply(e) => e.fmt(f),
            Error::AuthenticationFailed(e) => e.fmt(f),
            Error::InvalidTLSName => write!(f, "Invalid TLS name provided"),
            Error::MissingCredentials => write!(f, "Missing authentication credentials"),
            Error::MissingMailFrom => write!(f, "Missing message sender"),
            Error::MissingRcptTo => write!(f, "Missing message recipients"),
            Error::UnsupportedAuthMechanism => write!(
                f,
                "The server does no support any of the available authentication methods"
            ),
            Error::Timeout => write!(f, "Connection timeout"),
            Error::MissingStartTls => write!(f, "STARTTLS extension unavailable"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<base64::DecodeError> for Error {
    fn from(err: base64::DecodeError) -> Self {
        Error::Base64(err)
    }
}

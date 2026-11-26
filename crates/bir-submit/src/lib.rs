//! The client of the Buwiz relay server (protocol v2). A return leaves this
//! crate as plaintext XML sealed for buwiz-server, with a ticket and a
//! device signature, and comes back as a signed receipt.
//!
//! - [`Submitter`]: the device key and id in the OS credential store
//!   ([`secrets`]), tickets, the HPKE seal ([`seal`]), the v2 submission,
//!   the relay pipe ([`pipe`]) limited to the hosts of an offline-signed
//!   trust list ([`trust`]), receipt checks ([`receipt`]) and the account's
//!   TIN list ([`tins`]).
//! - [`TokenSource`]: the host app's one Buwiz sign-in. This crate never
//!   signs in, refreshes or stores a token: it asks the host for an access
//!   token on every call.
//! - [`wire`]: the few types and codes the server's answers are read with.
//!
//! This crate holds no BIR protocol constant, passphrase, endpoint or SFTP
//! code. Where the pipe may connect comes from the server's signed trust
//! list, trusted only through a pinned public key; the list also carries
//! the only sealing and receipt keys the app accepts.
//!
//! Ported from eBIRForms/eBIRForms `crates/bir-submit` at 14a0f63; see the
//! crate's README for what differs.

pub mod client;
pub mod device;
pub mod ios_storage;
pub mod pipe;
pub mod receipt;
pub mod seal;
pub mod secrets;
#[cfg(test)]
pub(crate) mod test_logs;
#[cfg(test)]
mod testkit;
pub mod tins;
pub mod trust;
pub mod wire;

pub use client::{
    CLIENT_HEADER, ClientError, DEFAULT_SERVER_URL, Note, PendingSend, Registered, SendError, Sent,
    Settled, SubmitConfig, Submitter, TokenSource,
};
pub use tins::{AccountTin, AccountTins, TinStatus, TinsError, TinsRead};
pub use wire::{Blame, TinKey, TinRefusal};

//! Encrypted SQLite database for taxpayer data.
//!
//! Stores profiles, form data, submission history.
//! Database is AES-256-GCM encrypted at rest.
//! Master key stored in OS keychain.

// TODO: Schema migrations
// TODO: Multi-taxpayer profile CRUD
// TODO: Submission history tracking

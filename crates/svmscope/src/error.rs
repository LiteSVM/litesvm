//! The crate's error type.
//!
//! One rule shapes this enum: a **reverting replay is not an error** — it's a
//! successful observation, reported inside [`crate::ReplayResult`]. An
//! `Err` here always means svmscope itself could not do what was asked: the RPC
//! failed, the transaction doesn't exist, a mutation targeted a missing account,
//! a named field couldn't be resolved. That distinction is what lets a scenario
//! that *expects* a revert never be satisfied by an infrastructure failure.

/// Convenience alias used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong short of the transaction itself reverting.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// An RPC request failed (network, rate limit, node error).
    #[error("rpc request failed: {0}")]
    Rpc(#[source] Box<solana_client::client_error::ClientError>),

    /// The RPC answered, but not with the shape we expect.
    #[error("unexpected rpc response: {0}")]
    MalformedRpcResponse(String),

    /// No transaction with this signature exists on the queried cluster.
    #[error("transaction not found: {0}")]
    TransactionNotFound(String),

    /// The address has no transaction history on the queried cluster.
    #[error("no transactions found for address {0}")]
    NoSignatures(String),

    /// The string is not a valid base58 Solana address.
    #[error("not a valid address: {0}")]
    InvalidAddress(String),

    /// The account does not exist on the queried cluster.
    #[error("account not found: {0}")]
    AccountNotFound(String),

    /// The program publishes no on-chain Anchor IDL (and none was supplied).
    #[error("{0} publishes no on-chain Anchor IDL — supply the IDL JSON to decode against it")]
    NoIdl(String),

    /// A mutation targeted an account the replay never loaded — usually a typo'd
    /// address. Reported as a hard error so an `expect: revert` scenario can
    /// never silently pass because of it.
    #[error("mutation targets an account not loaded in the replay: {0}")]
    MutationTargetMissing(String),

    /// A data patch would write past the end of the account's data.
    #[error(
        "patch out of range for {address}: offset {offset} + {len} bytes > account size {size}"
    )]
    PatchOutOfRange {
        /// The account the patch targeted.
        address: String,
        /// Byte offset where the patch begins.
        offset: usize,
        /// Length of the patch in bytes.
        len: usize,
        /// The account's actual data size in bytes.
        size: usize,
    },

    /// A named-field assert referenced a field the account's layout doesn't have.
    #[error("no field \"{field}\" on this {type_name} (fields: {})", available.join(", "))]
    UnknownField {
        /// The field name that was requested.
        field: String,
        /// The account type the field was looked up on.
        type_name: String,
        /// Every field name the layout actually has.
        available: Vec<String>,
    },

    /// A short field name matched several fields — the candidates disambiguate.
    #[error("\"{field}\" is ambiguous here — use one of: {}", candidates.join(", "))]
    AmbiguousField {
        /// The ambiguous short name.
        field: String,
        /// The fully qualified names it could mean.
        candidates: Vec<String>,
    },

    /// Only integer/bool fields can be compared numerically.
    #[error("{field} is a {ty} — only integer/bool fields can be asserted")]
    NonNumericField {
        /// The field that was asserted.
        field: String,
        /// Its actual (non-numeric) type.
        ty: String,
    },

    /// A named-field mutation's value doesn't fit the field's declared type.
    /// A hard error rather than a silent truncation.
    #[error("{value} does not fit field {field} ({ty})")]
    FieldValueOutOfRange {
        /// The field being mutated.
        field: String,
        /// The field's declared type (e.g. `u32`).
        ty: String,
        /// The rejected value.
        value: i128,
    },

    /// A field or offset read fell outside the account's data.
    #[error("{what} out of range (account data is {len} bytes)")]
    OutOfRange {
        /// What was being read (a field or byte range description).
        what: String,
        /// The account's data length in bytes.
        len: usize,
    },

    /// The account's bytes couldn't be decoded into named fields.
    #[error(
        "can't decode {address} into named fields (no known layout, and no IDL for its program {owner})"
    )]
    UndecodableAccount {
        /// The account that couldn't be decoded.
        address: String,
        /// The program that owns it.
        owner: String,
    },

    /// A transaction's wire bytes couldn't be decoded (bad base64 or bincode).
    #[error("could not decode transaction: {0}")]
    TxDecode(String),

    /// A fixture couldn't be serialized, parsed, or reloaded.
    #[error("fixture error: {0}")]
    Fixture(String),

    /// A suite/scenario/mutation specification was invalid (bad op, kind, hex…).
    #[error("{0}")]
    InvalidSpec(String),

    /// A submitted transaction did not reach confirmed commitment in time.
    #[error("timed out waiting for transaction confirmation: {signature}")]
    ConfirmationTimeout {
        /// The submitted transaction's signature.
        signature: String,
    },

    /// The transaction confirmed, but the RPC node had not indexed its
    /// `getTransaction` metadata before the wait timed out.
    #[error(
        "transaction {signature} was confirmed, but its metadata was unavailable before timeout"
    )]
    TransactionMetadataUnavailable {
        /// The confirmed transaction's signature.
        signature: String,
    },

    /// The requested method name does not exist in the program's IDL.
    #[error("method {method} was not found in the IDL for program {program}")]
    MethodNotFound {
        /// The program whose IDL was searched.
        program: String,
        /// The method name that wasn't found.
        method: String,
    },

    /// [`MethodBuilder::payer`](crate::MethodBuilder::payer) was never called.
    #[error("no payer was supplied for method {method}")]
    MissingPayer {
        /// The method being built.
        method: String,
    },

    /// The IDL requires an account that was never supplied to the builder.
    #[error("account {account} is required by method {method}")]
    MissingInstructionAccount {
        /// The method being built.
        method: String,
        /// The IDL name of the missing account.
        account: String,
    },

    /// The IDL marks an account as a signer, but no matching signer was added.
    #[error("missing signer for account {account} ({address})")]
    MissingSigner {
        /// The IDL name of the account that must sign.
        account: String,
        /// The address that has no registered signer.
        address: String,
    },

    /// The IDL requires an argument that was never supplied to the builder.
    #[error("argument {argument} is required by method {method}")]
    MissingArgument {
        /// The method being built.
        method: String,
        /// The IDL name of the missing argument.
        argument: String,
    },

    /// An argument was supplied that the method's IDL does not declare —
    /// usually a typo'd name.
    #[error("unknown argument {argument} for method {method}")]
    UnknownArgument {
        /// The method being built.
        method: String,
        /// The unrecognized argument name.
        argument: String,
    },

    /// An argument's JSON value couldn't be Borsh-encoded as its IDL type.
    #[error("could not encode argument {argument}: {reason}")]
    ArgumentEncoding {
        /// Dotted path to the failing value (e.g. `config.limits[1]`).
        argument: String,
        /// Why encoding failed.
        reason: String,
    },

    /// The signed transaction couldn't be assembled (e.g. a signature failed).
    #[error("could not construct transaction: {0}")]
    TransactionBuild(String),
}

impl Error {
    /// Wrap a solana client error (boxed — `ClientError` is large).
    pub(crate) fn rpc(e: solana_client::client_error::ClientError) -> Error {
        Error::Rpc(Box::new(e))
    }
}

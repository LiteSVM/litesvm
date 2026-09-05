//! Error naming: a program's custom error code → its IDL name, an Anchor
//! framework error, a native program's error, or the runtime's own
//! `InstructionError` — whichever applies, with a plain-language message.

use {
    crate::{native, DecodedError, IdlRegistry},
    litesvm_cpi_tree::FrameLog,
    solana_address::Address,
    solana_transaction_error::TransactionError,
};

/// Resolve a custom error `code` returned by `program`: the program's IDL
/// first, then the Anchor framework codes every Anchor program shares, then
/// the native programs' error enums. Always returns *something* — an
/// unresolvable code is still reported honestly as `Custom error N`.
pub fn error_name(registry: &IdlRegistry, program: &Address, code: u64) -> DecodedError {
    if let Some(e) = registry
        .get(program)
        .and_then(|m| crate::idl::error_for_code(m, *program, code))
    {
        return e;
    }
    if let Some((name, msg)) = framework_error(code) {
        return DecodedError {
            program: Some(*program),
            code: Some(code),
            name: name.into(),
            message: Some(msg.into()),
        };
    }
    if let Some((name, msg)) = native_error(&program.to_string(), code) {
        return DecodedError {
            program: Some(*program),
            code: Some(code),
            name: name.into(),
            message: Some(msg.into()),
        };
    }
    DecodedError {
        program: Some(*program),
        code: Some(code),
        name: format!("Custom error {code}"),
        message: None,
    }
}

/// The error behind a failed CPI frame: its `failed: <msg>` log line plus the
/// frame's own logs. An Anchor error log names the error exactly; a
/// `custom program error: 0x..` message resolves through [`error_name`]; a
/// runtime error resolves through its `InstructionError` name.
pub(crate) fn from_failure(
    registry: &IdlRegistry,
    program: &Address,
    msg: &str,
    logs: &[FrameLog],
) -> DecodedError {
    for log in logs {
        let FrameLog::Msg(text) = log else { continue };
        if let Some((name, code, message)) = parse_anchor_error(text) {
            return DecodedError {
                program: Some(*program),
                code: Some(code),
                name,
                message: Some(message),
            };
        }
    }
    if let Some(code) = custom_code(msg) {
        return error_name(registry, program, code);
    }
    if let Some((name, message)) = runtime_error(msg) {
        return DecodedError {
            program: Some(*program),
            code: None,
            name: name.into(),
            message: Some(message.into()),
        };
    }
    DecodedError {
        program: Some(*program),
        code: None,
        name: msg.to_string(),
        message: None,
    }
}

/// A transaction-level error, named. `InstructionError(i, Custom(c))` resolves
/// the code against the program at top-level instruction `i` when known.
pub fn describe_transaction_error(
    registry: &IdlRegistry,
    err: &TransactionError,
    program_at: impl Fn(usize) -> Option<Address>,
) -> DecodedError {
    let raw = format!("{err:?}");
    if let TransactionError::InstructionError(index, inner) = err {
        let inner_raw = format!("{inner:?}");
        if let (Some(program), Some(code)) = (program_at(*index as usize), custom_code(&inner_raw))
        {
            return error_name(registry, &program, code);
        }
        if let Some((name, message)) = runtime_error(&inner_raw) {
            return DecodedError {
                program: program_at(*index as usize),
                code: None,
                name: name.into(),
                message: Some(message.into()),
            };
        }
    }
    let (name, message) = runtime_error(&raw).unwrap_or((
        "Transaction failed",
        "The runtime rejected the transaction; see the raw error.",
    ));
    DecodedError {
        program: None,
        code: None,
        name: name.into(),
        message: Some(format!("{message} ({raw})")),
    }
}

/// `custom program error: 0x1773` / `Custom(6003)` → 6003.
fn custom_code(msg: &str) -> Option<u64> {
    if let Some(hex) = msg.split("custom program error: 0x").nth(1) {
        let digits: String = hex.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        return u64::from_str_radix(&digits, 16).ok();
    }
    msg.split("Custom(")
        .nth(1)
        .and_then(|s| s.split(')').next())
        .and_then(|s| s.parse().ok())
}

/// `AnchorError … Error Code: X. Error Number: N. Error Message: M.`
fn parse_anchor_error(log: &str) -> Option<(String, u64, String)> {
    if !log.contains("AnchorError") {
        return None;
    }
    let name = between(log, "Error Code: ", ".")?.to_string();
    let code = between(log, "Error Number: ", ".")?.trim().parse().ok()?;
    let message = log
        .split("Error Message: ")
        .nth(1)
        .map(|m| m.trim_end_matches('.').to_string())
        .unwrap_or_default();
    Some((name, code, message))
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let rest = s.split(start).nth(1)?;
    Some(rest.split(end).next()?.trim())
}

/// The constraint and account failures every Anchor program shares.
fn framework_error(code: u64) -> Option<(&'static str, &'static str)> {
    Some(match code {
        100 => (
            "InstructionMissing",
            "8-byte instruction identifier not provided",
        ),
        101 => (
            "InstructionFallbackNotFound",
            "Fallback functions are not supported",
        ),
        102 => (
            "InstructionDidNotDeserialize",
            "The program could not deserialize the given instruction",
        ),
        103 => (
            "InstructionDidNotSerialize",
            "The program could not serialize the given instruction",
        ),
        1000 => (
            "IdlInstructionStub",
            "The program was compiled without idl instructions",
        ),
        1001 => (
            "IdlInstructionInvalidProgram",
            "The transaction was given an invalid program for the IDL instruction",
        ),
        2000 => (
            "ConstraintMut",
            "A `mut` constraint was violated: an account expected to be writable was not",
        ),
        2001 => (
            "ConstraintHasOne",
            "A `has_one` constraint was violated: a stored field did not match the account passed",
        ),
        2002 => (
            "ConstraintSigner",
            "A `signer` constraint was violated: an account that must sign did not",
        ),
        2003 => ("ConstraintRaw", "A raw constraint was violated"),
        2004 => (
            "ConstraintOwner",
            "An `owner` constraint was violated: an account is owned by the wrong program",
        ),
        2005 => (
            "ConstraintRentExempt",
            "A rent-exemption constraint was violated",
        ),
        2006 => (
            "ConstraintSeeds",
            "A `seeds` constraint was violated: the PDA does not match the expected seeds",
        ),
        2007 => (
            "ConstraintExecutable",
            "An `executable` constraint was violated",
        ),
        2008 => (
            "ConstraintState",
            "Deprecated state constraint was violated",
        ),
        2009 => (
            "ConstraintAssociated",
            "An associated constraint was violated",
        ),
        2010 => (
            "ConstraintAssociatedInit",
            "An associated init constraint was violated",
        ),
        2011 => ("ConstraintClose", "A `close` constraint was violated"),
        2012 => (
            "ConstraintAddress",
            "An `address` constraint was violated: an account address did not match",
        ),
        2013 => ("ConstraintZero", "A `zero` constraint was violated"),
        2014 => (
            "ConstraintTokenMint",
            "A token mint constraint was violated: the token account's mint is wrong",
        ),
        2015 => (
            "ConstraintTokenOwner",
            "A token owner constraint was violated",
        ),
        2016 => (
            "ConstraintMintMintAuthority",
            "A mint mint-authority constraint was violated",
        ),
        2017 => (
            "ConstraintMintFreezeAuthority",
            "A mint freeze-authority constraint was violated",
        ),
        2018 => (
            "ConstraintMintDecimals",
            "A mint decimals constraint was violated",
        ),
        2019 => ("ConstraintSpace", "A `space` constraint was violated"),
        2020 => (
            "ConstraintAccountIsNone",
            "A required account for the constraint is None",
        ),
        2021 => (
            "ConstraintTokenTokenProgram",
            "A token account token-program constraint was violated",
        ),
        2022 => (
            "ConstraintMintTokenProgram",
            "A mint token-program constraint was violated",
        ),
        2023 => (
            "ConstraintAssociatedTokenTokenProgram",
            "An associated token account token-program constraint was violated",
        ),
        2500 => ("RequireViolated", "A `require!` expression was violated"),
        2501 => (
            "RequireEqViolated",
            "A `require_eq!` expression was violated",
        ),
        2502 => (
            "RequireKeysEqViolated",
            "A `require_keys_eq!` expression was violated",
        ),
        2503 => (
            "RequireNeqViolated",
            "A `require_neq!` expression was violated",
        ),
        2504 => (
            "RequireKeysNeqViolated",
            "A `require_keys_neq!` expression was violated",
        ),
        2505 => (
            "RequireGtViolated",
            "A `require_gt!` expression was violated",
        ),
        2506 => (
            "RequireGteViolated",
            "A `require_gte!` expression was violated",
        ),
        3000 => (
            "AccountDiscriminatorAlreadySet",
            "The account discriminator was already set on this account",
        ),
        3001 => (
            "AccountDiscriminatorNotFound",
            "No 8-byte discriminator on the account: likely uninitialized or the wrong account",
        ),
        3002 => (
            "AccountDiscriminatorMismatch",
            "The account's discriminator did not match: the wrong account type was passed",
        ),
        3003 => (
            "AccountDidNotDeserialize",
            "Failed to deserialize the account: its data does not match the expected layout",
        ),
        3004 => ("AccountDidNotSerialize", "Failed to serialize the account"),
        3005 => (
            "AccountNotEnoughKeys",
            "Not enough account keys given to the instruction",
        ),
        3006 => ("AccountNotMutable", "The given account is not mutable"),
        3007 => (
            "AccountOwnedByWrongProgram",
            "The account is owned by a different program than expected",
        ),
        3008 => ("InvalidProgramId", "A program id was not as expected"),
        3009 => (
            "InvalidProgramExecutable",
            "A program account is not executable",
        ),
        3010 => ("AccountNotSigner", "The given account did not sign"),
        3011 => (
            "AccountNotSystemOwned",
            "The account is not owned by the System program",
        ),
        3012 => (
            "AccountNotInitialized",
            "The program expected this account to already be initialized",
        ),
        3013 => (
            "AccountNotProgramData",
            "The given account is not a program data account",
        ),
        3014 => (
            "AccountNotAssociatedTokenAccount",
            "The given account is not the associated token account",
        ),
        3015 => (
            "AccountSysvarMismatch",
            "The given public key does not match the required sysvar",
        ),
        3016 => (
            "AccountReallocExceedsLimit",
            "The account reallocation exceeds the per-instruction growth limit",
        ),
        3017 => (
            "AccountDuplicateReallocs",
            "The account was reallocated more than once",
        ),
        4000 => (
            "StateInvalidAddress",
            "The given state account does not have the correct address",
        ),
        4100 => (
            "DeclaredProgramIdMismatch",
            "The declared program id does not match the actual program id",
        ),
        4101 => (
            "TryingToInitPayerAsProgramAccount",
            "You cannot/should not initialize the payer account as a program account",
        ),
        4102 => (
            "InvalidNumericConversion",
            "A numeric conversion overflowed or was invalid",
        ),
        5000 => (
            "Deprecated",
            "The API being used is deprecated and should no longer be used",
        ),
        _ => return None,
    })
}

/// Error enums of the native programs that publish no IDL: `SystemError`,
/// `TokenError` and the Associated Token program.
fn native_error(program: &str, code: u64) -> Option<(&'static str, &'static str)> {
    Some(match (program, code) {
        (native::SYSTEM, 0) => (
            "AccountAlreadyInUse",
            "The account to create already exists",
        ),
        (native::SYSTEM, 1) => (
            "ResultWithNegativeLamports",
            "The transfer would leave the source with a negative balance",
        ),
        (native::SYSTEM, 2) => (
            "InvalidProgramId",
            "The account cannot be assigned to that program",
        ),
        (native::SYSTEM, 3) => (
            "InvalidAccountDataLength",
            "The requested allocation size is not allowed",
        ),
        (native::SYSTEM, 4) => (
            "MaxSeedLengthExceeded",
            "A seed for a derived address is too long",
        ),
        (native::SYSTEM, 5) => (
            "AddressWithSeedMismatch",
            "The address does not derive from the given base and seed",
        ),
        (native::SYSTEM, 6) => (
            "NonceNoRecentBlockhashes",
            "The durable nonce could not be advanced",
        ),
        (native::SYSTEM, 7) => (
            "NonceBlockhashNotExpired",
            "The stored nonce is still valid, so it cannot be advanced yet",
        ),
        (native::SYSTEM, 8) => (
            "NonceUnexpectedBlockhashValue",
            "The transaction's blockhash does not match the nonce account",
        ),
        (native::TOKEN | native::TOKEN_2022, 0) => (
            "NotRentExempt",
            "The account lacks the lamports to be rent-exempt",
        ),
        (native::TOKEN | native::TOKEN_2022, 1) => (
            "InsufficientFunds",
            "The token account holds fewer tokens than the instruction moves",
        ),
        (native::TOKEN | native::TOKEN_2022, 2) => ("InvalidMint", "The mint account is not valid"),
        (native::TOKEN | native::TOKEN_2022, 3) => (
            "MintMismatch",
            "The token account belongs to a different mint",
        ),
        (native::TOKEN | native::TOKEN_2022, 4) => (
            "OwnerMismatch",
            "The signer is not the token account's owner or delegate",
        ),
        (native::TOKEN | native::TOKEN_2022, 5) => {
            ("FixedSupply", "The mint has no mint authority")
        }
        (native::TOKEN | native::TOKEN_2022, 6) => {
            ("AlreadyInUse", "The account is already initialized")
        }
        (native::TOKEN | native::TOKEN_2022, 7) => (
            "InvalidNumberOfProvidedSigners",
            "A multisig received the wrong number of signers",
        ),
        (native::TOKEN | native::TOKEN_2022, 8) => (
            "InvalidNumberOfRequiredSigners",
            "The multisig threshold is out of range",
        ),
        (native::TOKEN | native::TOKEN_2022, 9) => {
            ("UninitializedState", "The account has not been initialized")
        }
        (native::TOKEN | native::TOKEN_2022, 10) => (
            "NativeNotSupported",
            "This instruction does not apply to a native (wrapped SOL) account",
        ),
        (native::TOKEN | native::TOKEN_2022, 11) => (
            "NonNativeHasBalance",
            "A non-native account cannot be closed while it holds tokens",
        ),
        (native::TOKEN | native::TOKEN_2022, 12) => {
            ("InvalidInstruction", "The instruction data did not decode")
        }
        (native::TOKEN | native::TOKEN_2022, 13) => (
            "InvalidState",
            "The account is in a state that does not allow this operation",
        ),
        (native::TOKEN | native::TOKEN_2022, 14) => {
            ("Overflow", "An arithmetic operation overflowed")
        }
        (native::TOKEN | native::TOKEN_2022, 15) => (
            "AuthorityTypeNotSupported",
            "This authority type cannot be set on this account",
        ),
        (native::TOKEN | native::TOKEN_2022, 16) => {
            ("MintCannotFreeze", "The mint has no freeze authority")
        }
        (native::TOKEN | native::TOKEN_2022, 17) => {
            ("AccountFrozen", "The token account is frozen")
        }
        (native::TOKEN | native::TOKEN_2022, 18) => (
            "MintDecimalsMismatch",
            "The instruction's decimals do not match the mint",
        ),
        (native::TOKEN | native::TOKEN_2022, 19) => (
            "NonNativeNotSupported",
            "This instruction applies only to native (wrapped SOL) accounts",
        ),
        (native::ATA, 0) => (
            "InvalidOwner",
            "The associated token account's owner does not match",
        ),
        _ => return None,
    })
}

/// Plain-language names for the runtime's own `InstructionError` /
/// `TransactionError` variants, matched on the formatted error.
fn runtime_error(raw: &str) -> Option<(&'static str, &'static str)> {
    const TABLE: &[(&str, &str, &str)] = &[
        (
            "ProgramFailedToComplete",
            "ProgramFailedToComplete",
            "The program aborted: a panic, an out-of-bounds access, or an exceeded compute budget",
        ),
        (
            "ComputationalBudgetExceeded",
            "ComputationalBudgetExceeded",
            "The transaction used more compute units than it requested",
        ),
        (
            "InvalidAccountData",
            "InvalidAccountData",
            "An account's data was not what the program expected",
        ),
        (
            "AccountDataTooSmall",
            "AccountDataTooSmall",
            "An account is smaller than the program requires",
        ),
        (
            "MissingRequiredSignature",
            "MissingRequiredSignature",
            "An account that must sign did not",
        ),
        (
            "IncorrectProgramId",
            "IncorrectProgramId",
            "An account is owned by a different program than expected",
        ),
        (
            "InvalidArgument",
            "InvalidArgument",
            "The program rejected an instruction argument",
        ),
        (
            "InvalidInstructionData",
            "InvalidInstructionData",
            "The instruction data did not decode",
        ),
        (
            "PrivilegeEscalation",
            "PrivilegeEscalation",
            "A CPI tried to use an account as a signer or writable when the caller could not",
        ),
        (
            "ExternalAccountLamportSpend",
            "ExternalAccountLamportSpend",
            "A program debited lamports from an account it does not own",
        ),
        (
            "ReadonlyLamportChange",
            "ReadonlyLamportChange",
            "A program changed the balance of an account passed as read-only",
        ),
        (
            "ReadonlyDataModified",
            "ReadonlyDataModified",
            "A program wrote to an account passed as read-only",
        ),
        (
            "ExecutableDataModified",
            "ExecutableDataModified",
            "A program tried to write to an executable account",
        ),
        (
            "AccountBorrowFailed",
            "AccountBorrowFailed",
            "The same account was borrowed mutably twice in one instruction",
        ),
        (
            "UnbalancedInstruction",
            "UnbalancedInstruction",
            "Lamports were created or destroyed: the sums before and after differ",
        ),
        (
            "MaxSeedLengthExceeded",
            "MaxSeedLengthExceeded",
            "A PDA seed is longer than 32 bytes",
        ),
        (
            "InvalidSeeds",
            "InvalidSeeds",
            "The seeds do not derive the given program address",
        ),
        (
            "InvalidRealloc",
            "InvalidRealloc",
            "The account resize was rejected",
        ),
        (
            "AccountAlreadyInitialized",
            "AccountAlreadyInitialized",
            "The account was already initialized",
        ),
        (
            "UninitializedAccount",
            "UninitializedAccount",
            "The account has not been initialized",
        ),
        (
            "NotEnoughAccountKeys",
            "NotEnoughAccountKeys",
            "The instruction was given fewer accounts than it needs",
        ),
        (
            "InsufficientFundsForRent",
            "InsufficientFundsForRent",
            "An account would be left below the rent-exempt minimum",
        ),
        (
            "InsufficientFundsForFee",
            "InsufficientFundsForFee",
            "The fee payer cannot cover the transaction fee",
        ),
        (
            "InsufficientFunds",
            "InsufficientFunds",
            "An account does not hold enough lamports",
        ),
        (
            "BlockhashNotFound",
            "BlockhashNotFound",
            "The transaction's blockhash is not recent",
        ),
        (
            "AlreadyProcessed",
            "AlreadyProcessed",
            "This exact transaction was already executed",
        ),
        (
            "TooManyAccountLocks",
            "TooManyAccountLocks",
            "The transaction references more accounts than allowed",
        ),
        (
            "MaxLoadedAccountsDataSizeExceeded",
            "MaxLoadedAccountsDataSizeExceeded",
            "The accounts loaded exceed the requested data size limit",
        ),
        (
            "InvalidAddressLookupTableIndex",
            "InvalidAddressLookupTableIndex",
            "The transaction referenced a lookup table entry that is not active",
        ),
        (
            "AccountNotFound",
            "AccountNotFound",
            "An account the transaction needs does not exist",
        ),
        (
            "ProgramAccountNotFound",
            "ProgramAccountNotFound",
            "A program the transaction invokes does not exist",
        ),
        (
            "SignatureFailure",
            "SignatureFailure",
            "A signature did not verify",
        ),
        (
            "InvalidAccountOwner",
            "InvalidAccountOwner",
            "An account is owned by a program the instruction did not expect",
        ),
        (
            "ArithmeticOverflow",
            "ArithmeticOverflow",
            "An arithmetic operation overflowed",
        ),
        (
            "UnsupportedSysvar",
            "UnsupportedSysvar",
            "The program asked for a sysvar the runtime does not provide",
        ),
        (
            "IllegalOwner",
            "IllegalOwner",
            "The account's owner is not allowed",
        ),
    ];
    // The runtime writes both spellings: the `Debug` variant name
    // (`ProgramFailedToComplete`) in transaction errors and the `Display`
    // prose (`Program failed to complete`) in program logs. Compare with
    // case and spaces removed so one table serves both.
    let haystack = normalize(raw);
    TABLE
        .iter()
        .find(|(needle, _, _)| haystack.contains(&normalize(needle)))
        .map(|(_, t, d)| (*t, *d))
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use {super::*, std::str::FromStr};

    #[test]
    fn codes_resolve_in_priority_order() {
        let mut registry = IdlRegistry::new();
        let program = Address::new_unique();
        registry.insert(
            program,
            &serde_json::json!({ "errors": [{ "code": 6000, "name": "Mine", "msg": "own" }] }),
        );
        assert_eq!(error_name(&registry, &program, 6000).name, "Mine");
        assert_eq!(
            error_name(&registry, &program, 2005).name,
            "ConstraintRentExempt"
        );
        let token = Address::from_str(native::TOKEN).unwrap();
        assert_eq!(error_name(&registry, &token, 1).name, "InsufficientFunds");
        let e = error_name(&registry, &program, 99_999);
        assert_eq!(
            (e.name.as_str(), e.code),
            ("Custom error 99999", Some(99_999))
        );
    }

    #[test]
    fn failure_messages_parse() {
        assert_eq!(custom_code("custom program error: 0x1773"), Some(6003));
        assert_eq!(custom_code("InstructionError(2, Custom(6001))"), Some(6001));
        assert_eq!(custom_code("Program failed to complete"), None);
        let (name, code, msg) = parse_anchor_error(
            "AnchorError thrown in programs/x/src/lib.rs:100. Error Code: CliffNotReached. Error Number: 6003. Error Message: The vesting cliff has not been reached.",
        )
        .unwrap();
        assert_eq!(
            (name.as_str(), code, msg.as_str()),
            (
                "CliffNotReached",
                6003,
                "The vesting cliff has not been reached"
            )
        );
        let registry = IdlRegistry::new();
        let program = Address::new_unique();
        let e = from_failure(&registry, &program, "custom program error: 0x7d5", &[FrameLog::Msg("AnchorError caused by account: x. Error Code: ConstraintRentExempt. Error Number: 2005. Error Message: A rent exemption constraint was violated.".into())]);
        assert_eq!(
            (e.name.as_str(), e.code),
            ("ConstraintRentExempt", Some(2005))
        );
        let e = from_failure(&registry, &program, "Program failed to complete", &[]);
        assert_eq!(e.name, "ProgramFailedToComplete");
        assert_eq!(
            runtime_error("InstructionError(0, ComputationalBudgetExceeded)")
                .unwrap()
                .0,
            "ComputationalBudgetExceeded"
        );
        assert_eq!(
            runtime_error("exceeded CUs meter at BPF instruction").map(|r| r.0),
            None
        );
        assert_eq!(
            runtime_error("insufficient funds for fee").unwrap().0,
            "InsufficientFundsForFee"
        );
    }
}

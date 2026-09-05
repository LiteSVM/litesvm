//! Hand-written layouts for accounts whose programs publish no IDL: SPL Token
//! and Token-2022 accounts and mints, address lookup tables, stake accounts and
//! durable nonces.

use {
    crate::{DecodedAccount, Field},
    solana_address::Address,
};

pub(crate) const SPL_TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub(crate) const SPL_TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const STAKE_PROGRAM: &str = "Stake11111111111111111111111111111111111111";
const ALT_PROGRAM: &str = "AddressLookupTab1e1111111111111111111111111";
pub(crate) const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

fn read_u64(data: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(data[off..off + 8].try_into().unwrap())
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(data[off..off + 4].try_into().unwrap())
}

fn read_pubkey(data: &[u8], off: usize) -> String {
    let b: [u8; 32] = data[off..off + 32].try_into().unwrap();
    Address::from(b).to_string()
}

/// A `COption` is a 4-byte little-endian tag (0 = None, 1 = Some) then the payload.
fn coption_is_some(data: &[u8], off: usize) -> bool {
    read_u32(data, off) == 1
}

fn field(name: &str, offset: usize, ty: &str, size: usize, value: String) -> Field {
    Field {
        name: name.into(),
        offset,
        ty: ty.into(),
        size,
        value,
        note: None,
    }
}

/// Recognize and decode an account by owner and data. `None` for layouts we
/// don't know (callers then try the owner program's IDL).
///
/// The base Token and Token-2022 layouts are identical. A Token-2022 account
/// with extensions is longer than its base size; the type byte at offset 165
/// (Token-2022 pads mints past that offset precisely to disambiguate) says
/// whether it is a Mint (1) or an Account (2). A Multisig is exactly 355 bytes
/// and its byte 165 is an arbitrary signer-key byte, so it must not fall into
/// that rule.
pub fn decode_native_account(owner: &Address, data: &[u8]) -> Option<DecodedAccount> {
    match owner.to_string().as_str() {
        SPL_TOKEN | SPL_TOKEN_2022 => match data.len() {
            82 => Some(decode_mint(data)),
            165 => Some(decode_token_account(data)),
            355 => None,
            n if n > 165 => match data[165] {
                1 => Some(decode_mint(data)),
                2 => Some(decode_token_account(data)),
                _ => None,
            },
            _ => None,
        },
        ALT_PROGRAM => decode_lookup_table(data),
        STAKE_PROGRAM => decode_stake(data),
        SYSTEM_PROGRAM if data.len() == 80 => decode_nonce(data),
        _ => None,
    }
}

/// SPL Token account (Token or Token-2022 base layout, 165 bytes).
fn decode_token_account(data: &[u8]) -> DecodedAccount {
    let delegate = if coption_is_some(data, 72) {
        read_pubkey(data, 76)
    } else {
        "none".into()
    };
    let is_native = if coption_is_some(data, 109) {
        read_u64(data, 113).to_string()
    } else {
        "none".into()
    };
    let close_authority = if coption_is_some(data, 129) {
        read_pubkey(data, 133)
    } else {
        "none".into()
    };
    let state_code = data[108];
    let mut state = field("state", 108, "u8", 1, state_code.to_string());
    state.note = Some(
        match state_code {
            0 => "0 = uninitialized",
            1 => "1 = initialized",
            2 => "2 = frozen",
            _ => "unknown",
        }
        .into(),
    );
    DecodedAccount {
        type_name: "SPL Token Account".into(),
        fields: vec![
            field("mint", 0, "pubkey", 32, read_pubkey(data, 0)),
            field("owner", 32, "pubkey", 32, read_pubkey(data, 32)),
            field("amount", 64, "u64", 8, read_u64(data, 64).to_string()),
            field("delegate", 72, "coption-pubkey", 32, delegate),
            state,
            field("isNative", 109, "coption-u64", 8, is_native),
            field(
                "delegatedAmount",
                121,
                "u64",
                8,
                read_u64(data, 121).to_string(),
            ),
            field("closeAuthority", 129, "coption-pubkey", 32, close_authority),
        ],
    }
}

/// SPL Mint (Token or Token-2022 base layout, 82 bytes).
fn decode_mint(data: &[u8]) -> DecodedAccount {
    let mint_authority = if coption_is_some(data, 0) {
        read_pubkey(data, 4)
    } else {
        "none".into()
    };
    let freeze_authority = if coption_is_some(data, 46) {
        read_pubkey(data, 50)
    } else {
        "none".into()
    };
    DecodedAccount {
        type_name: "SPL Mint".into(),
        fields: vec![
            field("mintAuthority", 0, "coption-pubkey", 32, mint_authority),
            field("supply", 36, "u64", 8, read_u64(data, 36).to_string()),
            field("decimals", 44, "u8", 1, data[44].to_string()),
            field("isInitialized", 45, "bool", 1, (data[45] != 0).to_string()),
            field(
                "freezeAuthority",
                46,
                "coption-pubkey",
                32,
                freeze_authority,
            ),
        ],
    }
}

/// Address lookup table: a 56-byte header, then 32-byte addresses.
fn decode_lookup_table(data: &[u8]) -> Option<DecodedAccount> {
    if data.len() < 56 {
        return None;
    }
    // enum tag u32 @0, deactivation_slot @4, last_extended_slot @12,
    // last_extended_slot_start_index u8 @20, then `authority: Option<Pubkey>`
    // whose presence tag is at 21 (key at 22..54).
    let authority = if data[21] == 1 {
        read_pubkey(data, 22)
    } else {
        "none".into()
    };
    let addresses = (data.len() - 56) / 32;
    Some(DecodedAccount {
        type_name: "Address Lookup Table".into(),
        fields: vec![
            field(
                "deactivationSlot",
                4,
                "u64",
                8,
                read_u64(data, 4).to_string(),
            ),
            field(
                "lastExtendedSlot",
                12,
                "u64",
                8,
                read_u64(data, 12).to_string(),
            ),
            field(
                "lastExtendedSlotStartIndex",
                20,
                "u8",
                1,
                data[20].to_string(),
            ),
            field("authority", 22, "pubkey", 32, authority),
            field("addressCount", 56, "usize", 0, addresses.to_string()),
        ],
    })
}

/// Stake account: 4-byte state, Meta (authorized + lockup), then the Stake
/// struct when delegated.
fn decode_stake(data: &[u8]) -> Option<DecodedAccount> {
    if data.len() < 124 {
        return None;
    }
    let state = match read_u32(data, 0) {
        0 => "Uninitialized",
        1 => "Initialized",
        2 => "Stake",
        3 => "RewardsPool",
        _ => "unknown",
    };
    let mut fields = vec![
        field("state", 0, "enum", 4, state.into()),
        field(
            "rentExemptReserve",
            4,
            "u64",
            8,
            read_u64(data, 4).to_string(),
        ),
        field("authorizedStaker", 12, "pubkey", 32, read_pubkey(data, 12)),
        field(
            "authorizedWithdrawer",
            44,
            "pubkey",
            32,
            read_pubkey(data, 44),
        ),
        field(
            "lockupUnixTimestamp",
            76,
            "i64",
            8,
            (read_u64(data, 76) as i64).to_string(),
        ),
        field("lockupEpoch", 84, "u64", 8, read_u64(data, 84).to_string()),
        field("lockupCustodian", 92, "pubkey", 32, read_pubkey(data, 92)),
    ];
    if read_u32(data, 0) == 2 && data.len() >= 196 {
        fields.extend([
            field("voterPubkey", 124, "pubkey", 32, read_pubkey(data, 124)),
            field("stake", 156, "u64", 8, read_u64(data, 156).to_string()),
            field(
                "activationEpoch",
                164,
                "u64",
                8,
                read_u64(data, 164).to_string(),
            ),
            field(
                "deactivationEpoch",
                172,
                "u64",
                8,
                read_u64(data, 172).to_string(),
            ),
        ]);
    }
    Some(DecodedAccount {
        type_name: "Stake Account".into(),
        fields,
    })
}

/// Durable nonce account (system-owned, exactly 80 bytes).
fn decode_nonce(data: &[u8]) -> Option<DecodedAccount> {
    Some(DecodedAccount {
        type_name: "Nonce Account".into(),
        fields: vec![
            field("version", 0, "u32", 4, read_u32(data, 0).to_string()),
            field("state", 4, "u32", 4, read_u32(data, 4).to_string()),
            field("authority", 8, "pubkey", 32, read_pubkey(data, 8)),
            field("blockhash", 40, "pubkey", 32, read_pubkey(data, 40)),
            field(
                "lamportsPerSignature",
                72,
                "u64",
                8,
                read_u64(data, 72).to_string(),
            ),
        ],
    })
}

#[cfg(test)]
mod tests {
    use {super::*, std::str::FromStr};

    fn find<'a>(dec: &'a DecodedAccount, name: &str) -> &'a Field {
        dec.fields.iter().find(|f| f.name == name).expect("field")
    }

    #[test]
    fn alt_authority_presence_reads_the_option_tag_at_21_not_20() {
        let alt = Address::from_str(ALT_PROGRAM).unwrap();
        let mut data = vec![0u8; 56];
        data[20] = 7;
        data[21] = 1;
        for b in data.iter_mut().skip(22).take(32) {
            *b = 5;
        }
        assert_ne!(
            find(&decode_native_account(&alt, &data).unwrap(), "authority").value,
            "none"
        );
        let mut absent = vec![0u8; 56];
        absent[20] = 1;
        assert_eq!(
            find(&decode_native_account(&alt, &absent).unwrap(), "authority").value,
            "none"
        );
    }

    #[test]
    fn spl_multisig_is_not_misdecoded_as_mint_or_account() {
        let mut data = vec![0u8; 355];
        data[165] = 1;
        assert!(
            decode_native_account(&Address::from_str(SPL_TOKEN_2022).unwrap(), &data).is_none()
        );
    }

    #[test]
    fn token_account_fields() {
        let mut data = vec![0u8; 165];
        data[64..72].copy_from_slice(&5_000u64.to_le_bytes());
        data[108] = 2;
        let dec = decode_native_account(&Address::from_str(SPL_TOKEN).unwrap(), &data).unwrap();
        assert_eq!(dec.type_name, "SPL Token Account");
        assert_eq!(find(&dec, "amount").value, "5000");
        assert_eq!(find(&dec, "state").note.as_deref(), Some("2 = frozen"));
        assert_eq!(find(&dec, "delegate").value, "none");
    }
}

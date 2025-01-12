use {
    napi::bindgen_prelude::*,
    solana_sdk::{hash::Hash, pubkey::Pubkey},
    std::str::FromStr,
};

pub(crate) fn convert_pubkey(address: Uint8Array) -> Pubkey {
    Pubkey::try_from(address.as_ref()).unwrap()
}

pub(crate) fn try_parse_hash(raw: &str) -> Result<Hash> {
    Hash::from_str(raw).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to parse blockhash: {e}"),
        )
    })
}

use {napi::bindgen_prelude::*, solana_address::Address, solana_hash::Hash, std::str::FromStr};

pub(crate) fn convert_pubkey(address: &[u8]) -> Address {
    Address::try_from(address).unwrap()
}

pub(crate) fn try_parse_hash(raw: &str) -> Result<Hash> {
    Hash::from_str(raw).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to parse blockhash: {e}"),
        )
    })
}

pub(crate) fn bigint_to_u64(val: &BigInt) -> Result<u64> {
    let res = val.get_u64();
    if res.0 {
        return Err(Error::new(
            Status::GenericFailure,
            format!("Cannot convert negative bigint to u64: {val:?}"),
        ));
    }
    if !res.2 {
        return Err(Error::new(
            Status::GenericFailure,
            format!("Bigint too large for u64: {val:?}"),
        ));
    }
    Ok(res.1)
}

pub(crate) fn bigint_to_u128(val: &BigInt) -> Result<u128> {
    let res = val.get_u128();
    if res.0 {
        return Err(Error::new(
            Status::GenericFailure,
            format!("Cannot convert negative bigint to u128: {val:?}"),
        ));
    }
    if !res.2 {
        return Err(Error::new(
            Status::GenericFailure,
            format!("Bigint too large for u128: {val:?}"),
        ));
    }
    Ok(res.1)
}

pub(crate) fn bigint_to_i64(val: &BigInt) -> Result<i64> {
    let res = val.get_i64();
    if !res.1 {
        return Err(Error::new(
            Status::GenericFailure,
            format!("Bigint too large for i64: {val:?}"),
        ));
    }
    Ok(res.0)
}

pub(crate) fn bigint_to_usize(val: &BigInt) -> Result<usize> {
    let res = val.get_u64();
    if res.0 {
        return Err(Error::new(
            Status::GenericFailure,
            format!("Cannot convert negative bigint to usize: {val:?}"),
        ));
    }
    if !res.2 {
        return Err(Error::new(
            Status::GenericFailure,
            format!("Bigint too large for usize: {val:?}"),
        ));
    }
    let val_u64 = res.1;
    usize::try_from(val_u64).map_err(|_| {
        Error::new(
            Status::GenericFailure,
            format!("Bigint too large for usize: {val_u64}"),
        )
    })
}

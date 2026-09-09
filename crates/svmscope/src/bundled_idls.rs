//! IDLs that ship inside the crate for mainnet programs that never publish one
//! on-chain — native (Shank) programs and a few Anchor programs whose IDL only
//! lives in a repository. Consulted after the on-chain sources fail, so a
//! program that later publishes on-chain wins automatically.

use {
    serde_json::Value,
    std::{collections::HashMap, sync::LazyLock},
};

const BUNDLED: &[(&str, &str)] = &[
    (
        "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY",
        include_str!("../idls/phoenix.json"),
    ),
    (
        "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s",
        include_str!("../idls/token_metadata.json"),
    ),
    (
        "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK",
        include_str!("../idls/raydium_clmm.json"),
    ),
    (
        "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C",
        include_str!("../idls/raydium_cp_swap.json"),
    ),
    (
        "LanMV9sAd7wArD4vJFi2qDdfnVhFxYSUg6eADduJ3uj",
        include_str!("../idls/raydium_launchpad.json"),
    ),
];

static PARSED: LazyLock<HashMap<&'static str, Value>> = LazyLock::new(|| {
    BUNDLED
        .iter()
        .filter_map(|(id, text)| serde_json::from_str(text).ok().map(|v| (*id, v)))
        .collect()
});

/// The bundled IDL for a program id, if the crate ships one.
pub(crate) fn bundled_idl(program: &str) -> Option<Value> {
    PARSED.get(program).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_idl_parses_and_names_its_first_instruction() {
        assert_eq!(PARSED.len(), BUNDLED.len(), "a bundled IDL failed to parse");
        for (id, _) in BUNDLED {
            let idl = bundled_idl(id).unwrap();
            let ixs = crate::idl::instructions(&idl);
            assert!(!ixs.is_empty(), "{id} has no instructions");
        }
    }

    #[test]
    fn phoenix_swap_is_tag_zero() {
        let idl = bundled_idl("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY").unwrap();
        let ix = crate::idl::find_ix(&idl, &[0u8, 1, 2, 3]).expect("tag 0 matches");
        assert_eq!(ix.name.as_deref(), Some("Swap"));
        assert_eq!(crate::idl::disc_len(&ix), 1);
    }

    #[test]
    fn metaplex_create_metadata_v3_is_tag_33() {
        let idl = bundled_idl("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s").unwrap();
        let ix = crate::idl::find_ix(&idl, &[33u8, 0, 0]).expect("tag 33 matches");
        assert_eq!(ix.name.as_deref(), Some("CreateMetadataAccountV3"));
    }
}

use {
    crate::{to_string_js, util::bigint_to_u64},
    napi::bindgen_prelude::*,
    solana_account::Account as AccountOriginal,
    solana_address::Address,
};

#[derive(Debug, Clone)]
#[napi]
pub struct Account(pub(crate) AccountOriginal);

impl AsRef<AccountOriginal> for Account {
    fn as_ref(&self) -> &AccountOriginal {
        &self.0
    }
}

#[napi]
impl Account {
    #[napi(constructor)]
    pub fn new(
        lamports: BigInt,
        data: Uint8Array,
        owner: Uint8Array,
        executable: bool,
        rent_epoch: BigInt,
    ) -> Result<Self> {
        Ok(Self(AccountOriginal {
            lamports: bigint_to_u64(&lamports)?,
            data: data.to_vec(),
            owner: Address::try_from(owner.as_ref()).unwrap(),
            executable,
            rent_epoch: bigint_to_u64(&rent_epoch)?,
        }))
    }

    #[napi]
    pub fn lamports(&self) -> u64 {
        self.0.lamports
    }

    #[napi]
    pub fn data(&self) -> Uint8Array {
        Uint8Array::new(self.0.data.clone())
    }

    #[napi]
    pub fn owner(&self) -> Uint8Array {
        Uint8Array::new(self.0.owner.to_bytes().to_vec())
    }

    #[napi]
    pub fn executable(&self) -> bool {
        self.0.executable
    }

    #[napi]
    pub fn rent_epoch(&self) -> u64 {
        self.0.rent_epoch
    }
}

to_string_js!(Account);

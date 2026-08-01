#![allow(deprecated)]

use {
    crate::{
        to_string_js,
        util::{bigint_to_u64, bigint_to_usize},
    },
    napi::bindgen_prelude::*,
    solana_rent::{Rent as RentOriginal, ACCOUNT_STORAGE_OVERHEAD},
};

/// Configuration of network rent.
#[derive(Default, Debug)]
#[napi]
pub struct Rent(pub(crate) RentOriginal);

#[napi]
impl Rent {
    /// @param lamportsPerByteYear - Rental rate in lamports/byte-year.
    /// @param exemptionThreshold - Amount of time (in years) a balance must include rent for the account to be rent exempt.
    /// @param burnPercent - The percentage of collected rent that is burned.
    #[napi(constructor)]
    pub fn new(
        lamports_per_byte_year: BigInt,
        exemption_threshold: f64,
        burn_percent: u8,
    ) -> Result<Self> {
        Ok(Self(RentOriginal {
            lamports_per_byte: bigint_to_u64(&lamports_per_byte_year)?,
            exemption_threshold: exemption_threshold.to_le_bytes(),
            burn_percent,
        }))
    }

    /// Initialize rent with the default Solana settings.
    #[napi(factory, js_name = "default")]
    pub fn new_default() -> Self {
        Self::default()
    }

    /// Rental rate in lamports/byte-year.
    ///
    /// Note: since SIMD-0194 the underlying sysvar stores lamports/byte with
    /// the exemption threshold folded in (default 6960, threshold 1.0), so on
    /// current clusters this returns double the historical 3480 value.
    #[napi(getter)]
    pub fn lamports_per_byte_year(&self) -> u64 {
        self.0.lamports_per_byte
    }

    #[napi(setter)]
    pub fn set_lamports_per_byte_year(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.lamports_per_byte = bigint_to_u64(&val)?)
    }

    /// Amount of time (in years) a balance must include rent for the account to be rent exempt.
    #[napi(getter)]
    pub fn exemption_threshold(&self) -> f64 {
        f64::from_le_bytes(self.0.exemption_threshold)
    }

    #[napi(setter)]
    pub fn set_exemption_threshold(&mut self, val: f64) {
        self.0.exemption_threshold = val.to_le_bytes();
    }

    /// The percentage of collected rent that is burned.
    #[napi(getter)]
    pub fn burn_percent(&self) -> u8 {
        self.0.burn_percent
    }

    #[napi(setter)]
    pub fn set_burn_percent(&mut self, val: u8) {
        self.0.burn_percent = val;
    }

    /// Calculate how much rent to burn from the collected rent.
    ///
    /// The first value returned is the amount burned. The second is the amount
    /// to distribute to validators.
    ///
    /// @param rentCollected: The amount of rent collected.
    /// @returns The amount burned and the amount to distribute to validators.
    #[napi(ts_return_type = "[bigint, bigint]")]
    pub fn calculate_burn(&self, rent_collected: BigInt) -> Result<[u64; 2]> {
        let rent_collected = bigint_to_u64(&rent_collected)?;
        let burned = rent_collected.saturating_mul(u64::from(self.0.burn_percent)) / 100;
        Ok([burned, rent_collected.saturating_sub(burned)])
    }

    /// Minimum balance due for rent-exemption of a given account data size.
    ///
    /// Note: a stripped-down version of this calculation is used in
    /// ``calculate_split_rent_exempt_reserve`` in the stake program. When this
    /// function is updated, eg. when making rent variable, the stake program
    /// will need to be refactored.
    ///
    /// @param dataLen - The account data size.
    /// @returns The minimum balance due.
    #[napi]
    pub fn minimum_balance(&self, data_len: BigInt) -> Result<u64> {
        self.checked_minimum_balance(bigint_to_usize(&data_len)?)
    }

    /// Whether a given balance and data length would be exempt.
    #[napi]
    pub fn is_exempt(&self, balance: BigInt, data_len: BigInt) -> Result<bool> {
        Ok(
            bigint_to_u64(&balance)?
                >= self.checked_minimum_balance(bigint_to_usize(&data_len)?)?,
        )
    }

    /// Rent due on account's data length with balance.
    ///
    /// @param balance - The account balance.
    /// @param dataLen - The account data length.
    /// @param yearsElapsed - Time elapsed in years.
    /// @returns The rent due.
    #[napi]
    pub fn due(
        &self,
        balance: BigInt,
        data_len: BigInt,
        years_elapsed: f64,
    ) -> Result<Option<u64>> {
        let balance = bigint_to_u64(&balance)?;
        let data_len = bigint_to_usize(&data_len)?;
        if balance >= self.checked_minimum_balance(data_len)? {
            Ok(None)
        } else {
            Ok(Some(self.due_amount_inner(data_len as u64, years_elapsed)))
        }
    }

    /// Rent due for account that is known to be not exempt.
    ///
    /// Note: since SIMD-0194 the underlying rate is lamports/byte with the
    /// exemption threshold folded in, so on current clusters this returns
    /// double the pre-SIMD-0194 amount for the same inputs.
    ///
    /// @param dataLen - The account data length.
    /// @param yearsElapsed - Time elapsed in years.
    /// @returns The amount due.
    #[napi]
    pub fn due_amount(&self, data_len: BigInt, years_elapsed: f64) -> Result<u64> {
        Ok(self.due_amount_inner(bigint_to_usize(&data_len)? as u64, years_elapsed))
    }

    /// Creates a `Rent` that charges no lamports.
    ///
    /// This is used for testing.
    ///
    #[napi(factory)]
    pub fn free() -> Self {
        Self(RentOriginal::free())
    }

    /// Creates a `Rent` that is scaled based on the number of slots in an epoch.
    ///
    /// This is used for testing.
    ///
    /// @deprecated Epoch-based rent scaling was removed upstream (SIMD-0194);
    /// the argument is ignored and the default rent is returned. The old
    /// scaling never affected the rent-exempt minimum balance.
    #[napi(factory)]
    pub fn with_slots_per_epoch(_slots_per_epoch: BigInt) -> Result<Self> {
        Ok(Self::default())
    }

    fn checked_minimum_balance(&self, data_len: usize) -> Result<u64> {
        self.0.try_minimum_balance(data_len).ok_or_else(|| {
            Error::new(
                Status::InvalidArg,
                "maximum permitted data length exceeded".to_string(),
            )
        })
    }

    fn due_amount_inner(&self, data_len: u64, years_elapsed: f64) -> u64 {
        (((ACCOUNT_STORAGE_OVERHEAD + data_len) * self.0.lamports_per_byte) as f64 * years_elapsed)
            as u64
    }
}

to_string_js!(Rent);

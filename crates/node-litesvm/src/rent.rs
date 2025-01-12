use {
    napi::bindgen_prelude::*,
    solana_sdk::rent::{Rent as RentOriginal, RentDue},
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
    pub fn new(lamports_per_byte_year: BigInt, exemption_threshold: f64, burn_percent: u8) -> Self {
        Self(RentOriginal {
            lamports_per_byte_year: lamports_per_byte_year.get_u64().1,
            exemption_threshold,
            burn_percent,
        })
    }

    /// Initialize rent with the default Solana settings.
    #[napi(factory, js_name = "default")]
    pub fn new_default() -> Self {
        Self::default()
    }

    /// Rental rate in lamports/byte-year.
    #[napi(getter)]
    pub fn lamports_per_byte_year(&self) -> u64 {
        self.0.lamports_per_byte_year
    }

    /// Amount of time (in years) a balance must include rent for the account to be rent exempt.
    #[napi(getter)]
    pub fn exemption_threshold(&self) -> f64 {
        self.0.exemption_threshold
    }

    /// The percentage of collected rent that is burned.
    #[napi(getter)]
    pub fn burn_percent(&self) -> u8 {
        self.0.burn_percent
    }

    /// Calculate how much rent to burn from the collected rent.
    ///
    /// The first value returned is the amount burned. The second is the amount
    /// to distribute to validators.
    ///
    /// @param rentCollected: The amount of rent collected.
    /// @returns The amount burned and the amount to distribute to validators.
    #[napi]
    pub fn calculate_burn(&self, env: Env, rent_collected: BigInt) -> Array {
        let mut arr = env.create_array(2).unwrap();
        let res = self.0.calculate_burn(rent_collected.get_u64().1);
        arr.insert(res.0).unwrap();
        arr.insert(res.1).unwrap();
        arr
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
    pub fn minimum_balance(&self, data_len: BigInt) -> u64 {
        self.0.minimum_balance(data_len.get_u64().1 as usize)
    }

    /// Whether a given balance and data length would be exempt.
    #[napi]
    pub fn is_exempt(&self, balance: BigInt, data_len: BigInt) -> bool {
        self.0
            .is_exempt(balance.get_u64().1, data_len.get_u64().1 as usize)
    }

    /// Rent due on account's data length with balance.
    ///
    /// @param balance - The account balance.
    /// @param dataLen - The account data length.
    /// @param yearsElapsed - Time elapsed in years.
    /// @returns The rent due.
    #[napi]
    pub fn due(&self, balance: BigInt, data_len: BigInt, years_elapsed: f64) -> Option<u64> {
        match self.0.due(
            balance.get_u64().1,
            data_len.get_u64().1 as usize,
            years_elapsed,
        ) {
            RentDue::Exempt => None,
            RentDue::Paying(x) => Some(x),
        }
    }

    /// Rent due for account that is known to be not exempt.
    ///
    /// @param dataLen - The account data length.
    /// @param yearsElapsed - Time elapsed in years.
    /// @returns The amount due.
    #[napi]
    pub fn due_amount(&self, data_len: BigInt, years_elapsed: f64) -> u64 {
        self.0
            .due_amount(data_len.get_u64().1 as usize, years_elapsed)
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
    #[napi(factory)]
    pub fn with_slots_per_epoch(slots_per_epoch: BigInt) -> Self {
        Self(RentOriginal::with_slots_per_epoch(
            slots_per_epoch.get_u64().1,
        ))
    }

    #[napi(js_name = "toString")]
    pub fn js_to_string(&self) -> String {
        format!("{self:?}")
    }
}

/// Tracks compute unit consumption across transactions
#[derive(Default)]
pub struct ComputeTracker {
    /// Total compute units consumed across all transactions
    total_compute_units_consumed: u64,
    /// Number of transactions processed
    transaction_count: u64,
    /// Maximum compute units consumed by any single transaction
    max_compute_units: u64,
    /// Minimum compute units consumed by any single transaction (initialized to u64::MAX)
    min_compute_units: u64,
}

impl ComputeTracker {
    /// Creates a new compute tracker
    pub fn new() -> Self {
        Self {
            total_compute_units_consumed: 0,
            transaction_count: 0,
            max_compute_units: 0,
            min_compute_units: u64::MAX,
        }
    }

    /// Records compute units consumed by a transaction
    pub fn record_transaction(&mut self, compute_units: u64) {
        self.total_compute_units_consumed += compute_units;
        self.transaction_count += 1;
        self.max_compute_units = self.max_compute_units.max(compute_units);
        self.min_compute_units = self.min_compute_units.min(compute_units);
    }

    /// Gets the average compute units per transaction
    pub fn average_compute_units(&self) -> f64 {
        if self.transaction_count == 0 {
            0.0
        } else {
            self.total_compute_units_consumed as f64 / self.transaction_count as f64
        }
    }

    /// Gets the total compute units consumed
    pub fn total_compute_units(&self) -> u64 {
        self.total_compute_units_consumed
    }

    /// Gets the maximum compute units consumed by any transaction
    pub fn max_compute_units(&self) -> u64 {
        self.max_compute_units
    }

    /// Gets the minimum compute units consumed by any transaction
    pub fn min_compute_units(&self) -> u64 {
        if self.transaction_count == 0 {
            0
        } else {
            self.min_compute_units
        }
    }

    /// Gets the number of transactions processed
    pub fn transaction_count(&self) -> u64 {
        self.transaction_count
    }

    /// Resets all tracking metrics
    pub fn reset(&mut self) {
        self.total_compute_units_consumed = 0;
        self.transaction_count = 0;
        self.max_compute_units = 0;
        self.min_compute_units = u64::MAX;
    }
} 
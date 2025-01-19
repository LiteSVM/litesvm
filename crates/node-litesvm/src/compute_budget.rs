use {
    crate::util::{bigint_to_u64, bigint_to_usize},
    napi::bindgen_prelude::*,
    solana_compute_budget::compute_budget::ComputeBudget as ComputeBudgetOriginal,
};

#[napi]
pub struct ComputeBudget(pub(crate) ComputeBudgetOriginal);

#[napi]
impl ComputeBudget {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self(ComputeBudgetOriginal::default())
    }

    #[napi(getter)]
    pub fn compute_unit_limit(&self) -> u64 {
        self.0.compute_unit_limit
    }

    #[napi(setter)]
    pub fn set_compute_unit_limit(&mut self, limit: BigInt) -> Result<()> {
        Ok(self.0.compute_unit_limit = bigint_to_u64(&limit)?)
    }

    #[napi(setter)]
    pub fn set_log_64_units(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.log_64_units = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn log_64_units(&self) -> u64 {
        self.0.log_64_units
    }
    #[napi(setter)]
    pub fn set_create_program_address_units(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.create_program_address_units = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn create_program_address_units(&self) -> u64 {
        self.0.create_program_address_units
    }
    #[napi(setter)]
    pub fn set_invoke_units(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.invoke_units = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn invoke_units(&self) -> u64 {
        self.0.invoke_units
    }
    #[napi(setter)]
    pub fn set_max_instruction_stack_depth(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.max_instruction_stack_depth = bigint_to_usize(&val)?)
    }
    #[napi(getter)]
    pub fn max_instruction_stack_depth(&self) -> usize {
        self.0.max_instruction_stack_depth
    }
    #[napi(setter)]
    pub fn set_max_instruction_trace_length(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.max_instruction_trace_length = bigint_to_usize(&val)?)
    }
    #[napi(getter)]
    pub fn max_instruction_trace_length(&self) -> usize {
        self.0.max_instruction_trace_length
    }
    #[napi(setter)]
    pub fn set_sha256_base_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.sha256_base_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn sha256_base_cost(&self) -> u64 {
        self.0.sha256_base_cost
    }
    #[napi(setter)]
    pub fn set_sha256_byte_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.sha256_byte_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn sha256_byte_cost(&self) -> u64 {
        self.0.sha256_byte_cost
    }
    #[napi(setter)]
    pub fn set_sha256_max_slices(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.sha256_max_slices = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn sha256_max_slices(&self) -> u64 {
        self.0.sha256_max_slices
    }
    #[napi(setter)]
    pub fn set_max_call_depth(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.max_call_depth = bigint_to_usize(&val)?)
    }
    #[napi(getter)]
    pub fn max_call_depth(&self) -> usize {
        self.0.max_call_depth
    }
    #[napi(setter)]
    pub fn set_stack_frame_size(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.stack_frame_size = bigint_to_usize(&val)?)
    }
    #[napi(getter)]
    pub fn stack_frame_size(&self) -> usize {
        self.0.stack_frame_size
    }
    #[napi(setter)]
    pub fn set_log_pubkey_units(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.log_pubkey_units = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn log_pubkey_units(&self) -> u64 {
        self.0.log_pubkey_units
    }
    #[napi(setter)]
    pub fn set_max_cpi_instruction_size(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.max_cpi_instruction_size = bigint_to_usize(&val)?)
    }
    #[napi(getter)]
    pub fn max_cpi_instruction_size(&self) -> usize {
        self.0.max_cpi_instruction_size
    }
    #[napi(setter)]
    pub fn set_cpi_bytes_per_unit(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.cpi_bytes_per_unit = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn cpi_bytes_per_unit(&self) -> u64 {
        self.0.cpi_bytes_per_unit
    }
    #[napi(setter)]
    pub fn set_sysvar_base_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.sysvar_base_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn sysvar_base_cost(&self) -> u64 {
        self.0.sysvar_base_cost
    }
    #[napi(setter)]
    pub fn set_secp256k1_recover_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.secp256k1_recover_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn secp256k1_recover_cost(&self) -> u64 {
        self.0.secp256k1_recover_cost
    }
    #[napi(setter)]
    pub fn set_syscall_base_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.syscall_base_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn syscall_base_cost(&self) -> u64 {
        self.0.syscall_base_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_edwards_validate_point_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_edwards_validate_point_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_edwards_validate_point_cost(&self) -> u64 {
        self.0.curve25519_edwards_validate_point_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_edwards_add_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_edwards_add_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_edwards_add_cost(&self) -> u64 {
        self.0.curve25519_edwards_add_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_edwards_subtract_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_edwards_subtract_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_edwards_subtract_cost(&self) -> u64 {
        self.0.curve25519_edwards_subtract_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_edwards_multiply_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_edwards_multiply_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_edwards_multiply_cost(&self) -> u64 {
        self.0.curve25519_edwards_multiply_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_edwards_msm_base_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_edwards_msm_base_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_edwards_msm_base_cost(&self) -> u64 {
        self.0.curve25519_edwards_msm_base_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_edwards_msm_incremental_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_edwards_msm_incremental_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_edwards_msm_incremental_cost(&self) -> u64 {
        self.0.curve25519_edwards_msm_incremental_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_ristretto_validate_point_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_ristretto_validate_point_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_ristretto_validate_point_cost(&self) -> u64 {
        self.0.curve25519_ristretto_validate_point_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_ristretto_add_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_ristretto_add_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_ristretto_add_cost(&self) -> u64 {
        self.0.curve25519_ristretto_add_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_ristretto_subtract_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_ristretto_subtract_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_ristretto_subtract_cost(&self) -> u64 {
        self.0.curve25519_ristretto_subtract_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_ristretto_multiply_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_ristretto_multiply_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_ristretto_multiply_cost(&self) -> u64 {
        self.0.curve25519_ristretto_multiply_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_ristretto_msm_base_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_ristretto_msm_base_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_ristretto_msm_base_cost(&self) -> u64 {
        self.0.curve25519_ristretto_msm_base_cost
    }
    #[napi(setter)]
    pub fn set_curve25519_ristretto_msm_incremental_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.curve25519_ristretto_msm_incremental_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn curve25519_ristretto_msm_incremental_cost(&self) -> u64 {
        self.0.curve25519_ristretto_msm_incremental_cost
    }
    #[napi(setter)]
    pub fn set_heap_size(&mut self, val: u32) {
        self.0.heap_size = val;
    }
    #[napi(getter)]
    pub fn heap_size(&self) -> u32 {
        self.0.heap_size
    }
    #[napi(setter)]
    pub fn set_heap_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.heap_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn heap_cost(&self) -> u64 {
        self.0.heap_cost
    }
    #[napi(setter)]
    pub fn set_mem_op_base_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.mem_op_base_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn mem_op_base_cost(&self) -> u64 {
        self.0.mem_op_base_cost
    }
    #[napi(setter)]
    pub fn set_alt_bn128_addition_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.alt_bn128_addition_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn alt_bn128_addition_cost(&self) -> u64 {
        self.0.alt_bn128_addition_cost
    }
    #[napi(setter)]
    pub fn set_alt_bn128_multiplication_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.alt_bn128_multiplication_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn alt_bn128_multiplication_cost(&self) -> u64 {
        self.0.alt_bn128_multiplication_cost
    }
    #[napi(setter)]
    pub fn set_alt_bn128_pairing_one_pair_cost_first(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.alt_bn128_pairing_one_pair_cost_first = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn alt_bn128_pairing_one_pair_cost_first(&self) -> u64 {
        self.0.alt_bn128_pairing_one_pair_cost_first
    }
    #[napi(setter)]
    pub fn set_alt_bn128_pairing_one_pair_cost_other(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.alt_bn128_pairing_one_pair_cost_other = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn alt_bn128_pairing_one_pair_cost_other(&self) -> u64 {
        self.0.alt_bn128_pairing_one_pair_cost_other
    }
    #[napi(setter)]
    pub fn set_big_modular_exponentiation_base_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.big_modular_exponentiation_base_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn big_modular_exponentiation_base_cost(&self) -> u64 {
        self.0.big_modular_exponentiation_base_cost
    }
    #[napi(setter)]
    pub fn set_big_modular_exponentiation_cost_divisor(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.big_modular_exponentiation_cost_divisor = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn big_modular_exponentiation_cost_divisor(&self) -> u64 {
        self.0.big_modular_exponentiation_cost_divisor
    }
    #[napi(setter)]
    pub fn set_poseidon_cost_coefficient_a(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.poseidon_cost_coefficient_a = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn poseidon_cost_coefficient_a(&self) -> u64 {
        self.0.poseidon_cost_coefficient_a
    }
    #[napi(setter)]
    pub fn set_poseidon_cost_coefficient_c(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.poseidon_cost_coefficient_c = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn poseidon_cost_coefficient_c(&self) -> u64 {
        self.0.poseidon_cost_coefficient_c
    }
    #[napi(setter)]
    pub fn set_get_remaining_compute_units_cost(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.get_remaining_compute_units_cost = bigint_to_u64(&val)?)
    }
    #[napi(getter, js_name = "getRemainingComputeUnitsCost")]
    pub fn remaining_compute_units_cost(&self) -> u64 {
        self.0.get_remaining_compute_units_cost
    }
    #[napi(setter)]
    pub fn set_alt_bn128_g1_compress(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.alt_bn128_g1_compress = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn alt_bn128_g1_compress(&self) -> u64 {
        self.0.alt_bn128_g1_compress
    }
    #[napi(setter)]
    pub fn set_alt_bn128_g1_decompress(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.alt_bn128_g1_decompress = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn alt_bn128_g1_decompress(&self) -> u64 {
        self.0.alt_bn128_g1_decompress
    }
    #[napi(setter)]
    pub fn set_alt_bn128_g2_compress(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.alt_bn128_g2_compress = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn alt_bn128_g2_compress(&self) -> u64 {
        self.0.alt_bn128_g2_compress
    }
    #[napi(setter)]
    pub fn set_alt_bn128_g2_decompress(&mut self, val: BigInt) -> Result<()> {
        Ok(self.0.alt_bn128_g2_decompress = bigint_to_u64(&val)?)
    }
    #[napi(getter)]
    pub fn alt_bn128_g2_decompress(&self) -> u64 {
        self.0.alt_bn128_g2_decompress
    }
}

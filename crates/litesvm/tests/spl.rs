use {
    arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs},
    ethnum::U256,
    jupnet_sdk::{
        instruction::{AccountMeta, Instruction},
        program_error::ProgramError,
        program_option::COption,
        program_pack::{IsInitialized, Pack, Sealed},
        pubkey::Pubkey,
        rent::Rent,
        signer::{keypair::Keypair, Signer},
        system_instruction,
        transaction::Transaction,
    },
    litesvm::LiteSVM,
};

const SPL_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

fn unpack_coption_key(src: &[u8; 36]) -> Result<COption<Pubkey>, ProgramError> {
    let (tag, body) = array_refs![src, 4, 32];
    match *tag {
        [0, 0, 0, 0] => Ok(COption::None),
        [1, 0, 0, 0] => Ok(COption::Some(Pubkey::new_from_array(*body))),
        _ => Err(ProgramError::InvalidAccountData),
    }
}

fn pack_coption_key(src: &COption<Pubkey>, dst: &mut [u8; 36]) {
    let (tag, body) = mut_array_refs![dst, 4, 32];
    match src {
        COption::Some(key) => {
            *tag = [1, 0, 0, 0];
            body.copy_from_slice(key.as_ref());
        }
        COption::None => {
            *tag = [0; 4];
        }
    }
}

/// Mint data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mint {
    /// Optional authority used to mint new tokens. The mint authority may only
    /// be provided during mint creation. If no mint authority is present
    /// then the mint has a fixed supply and no further tokens may be
    /// minted.
    pub mint_authority: COption<Pubkey>,
    /// Total supply of tokens.
    pub supply: U256,
    /// Number of base 10 digits to the right of the decimal place.
    pub decimals: u8,
    /// Is `true` if this structure has been initialized
    pub is_initialized: bool,
    /// Optional authority to freeze token accounts.
    pub freeze_authority: COption<Pubkey>,
}
impl Sealed for Mint {}
impl IsInitialized for Mint {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}
impl Pack for Mint {
    const LEN: usize = 106;
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, 106];
        let (mint_authority, supply, decimals, is_initialized, freeze_authority) =
            array_refs![src, 36, 32, 1, 1, 36];
        let mint_authority = unpack_coption_key(mint_authority)?;
        let supply = U256::from_le_bytes(*supply);
        let decimals = decimals[0];
        let is_initialized = match is_initialized {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        let freeze_authority = unpack_coption_key(freeze_authority)?;
        Ok(Mint {
            mint_authority,
            supply,
            decimals,
            is_initialized,
            freeze_authority,
        })
    }
    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, 106];
        let (
            mint_authority_dst,
            supply_dst,
            decimals_dst,
            is_initialized_dst,
            freeze_authority_dst,
        ) = mut_array_refs![dst, 36, 32, 1, 1, 36];
        let &Mint {
            ref mint_authority,
            supply,
            decimals,
            is_initialized,
            ref freeze_authority,
        } = self;
        pack_coption_key(mint_authority, mint_authority_dst);
        *supply_dst = supply.to_le_bytes();
        decimals_dst[0] = decimals;
        is_initialized_dst[0] = is_initialized as u8;
        pack_coption_key(freeze_authority, freeze_authority_dst);
    }
}

#[test]
fn spl_token() {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let mint_kp = Keypair::new();
    let mint_pk = mint_kp.pubkey();
    let mint_len = Mint::LEN;

    svm.airdrop(&payer_pk, 1000000000).unwrap();

    let create_acc_ins = system_instruction::create_account(
        &payer_pk,
        &mint_pk,
        svm.minimum_balance_for_rent_exemption(mint_len),
        mint_len as u64,
        &SPL_PROGRAM_ID,
    );

    let mut data = Vec::with_capacity(35);
    data.push(20);
    data.push(8);
    data.extend_from_slice(&payer_pk.to_bytes());
    data.push(0);
    let init_mint_ins = Instruction {
        program_id: SPL_PROGRAM_ID,
        accounts: vec![AccountMeta::new(mint_pk, false)],
        data,
    };

    let balance_before = svm.get_balance(&payer_pk).unwrap();
    let expected_fee = 2 * 5000; // two signers
    let tx_result = svm.send_transaction(Transaction::new_signed_with_payer(
        &[create_acc_ins, init_mint_ins],
        Some(&payer_pk),
        &[&payer_kp, &mint_kp],
        svm.latest_blockhash(),
    ));
    assert!(tx_result.is_ok());
    let expected_rent = svm.get_sysvar::<Rent>().minimum_balance(Mint::LEN);
    let balance_after = svm.get_balance(&payer_pk).unwrap();

    assert_eq!(balance_before - balance_after, expected_rent + expected_fee);

    let mint_acc = svm.get_account(&mint_kp.pubkey());
    let mint = Mint::unpack(&mint_acc.unwrap().data).unwrap();

    assert_eq!(mint.decimals, 8);
    assert_eq!(mint.mint_authority, Some(payer_pk).into());
}

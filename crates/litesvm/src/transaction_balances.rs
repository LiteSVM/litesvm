use {
    crate::{
        types::{ExecutionResult, TokenBalance, TransactionBalances},
        LiteSVM,
    },
    solana_account::{AccountSharedData, ReadableAccount},
    solana_address::Address,
    solana_message::SanitizedMessage,
    spl_generic_token::{generic_token, is_known_spl_token_id},
};

pub(crate) fn collect_balances<'a>(
    message: &SanitizedMessage,
    get_account: impl Fn(&Address) -> Option<&'a AccountSharedData>,
) -> (Vec<u64>, Vec<TokenBalance>) {
    let keys = message.account_keys();
    let has_token_program = keys.iter().any(is_token_program);
    let mut native = Vec::with_capacity(keys.len());
    let mut tokens = Vec::new();
    for (index, key) in keys.iter().enumerate() {
        let account = get_account(key).filter(|account| account.lamports() != 0);
        native.push(account.map_or(0, ReadableAccount::lamports));
        if has_token_program && !message.is_invoked(index) && !is_token_program(key) {
            if let Some(balance) =
                account.and_then(|account| token_balance(index, account, &get_account))
            {
                tokens.push(balance);
            }
        }
    }
    (native, tokens)
}

fn is_token_program(key: &Address) -> bool {
    is_known_spl_token_id(&key.to_bytes().into())
}

fn token_balance<'a>(
    index: usize,
    account: &AccountSharedData,
    get_account: &impl Fn(&Address) -> Option<&'a AccountSharedData>,
) -> Option<TokenBalance> {
    let program_id = *account.owner();
    let generic_program_id = program_id.to_bytes().into();
    let token = generic_token::Account::unpack(account.data(), &generic_program_id)?;
    let mint = Address::new_from_array(token.mint.to_bytes());
    let mint_account = get_account(&mint)?;
    if mint_account.lamports() == 0 || *mint_account.owner() != program_id {
        return None;
    }
    let mint_info = generic_token::Mint::unpack(mint_account.data(), &generic_program_id)?;
    Some(TokenBalance {
        account_index: index.try_into().ok()?,
        mint,
        owner: Address::new_from_array(token.owner.to_bytes()),
        amount: token.amount,
        decimals: mint_info.decimals,
        program_id,
    })
}

impl LiteSVM {
    pub(crate) fn record_post_balances(
        &self,
        message: &SanitizedMessage,
        result: &ExecutionResult,
        balances: &mut TransactionBalances,
    ) {
        // Failed execution writes are rolled back; only the fee debit survives.
        let post_accounts: crate::HashMap<_, _> = result
            .post_accounts
            .iter()
            .filter(|_| result.tx_result.is_ok())
            .map(|(key, account)| (key, account))
            .collect();
        (balances.post_balances, balances.post_token_balances) = collect_balances(message, |key| {
            // A mint initialized in this transaction must be read from its post-state too.
            post_accounts
                .get(key)
                .copied()
                .or_else(|| self.accounts.get_account_ref(key))
        });
    }
}

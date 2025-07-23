use alloy::sol;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract DepositContract {
        event DepositEvent(
            bytes pubkey,
            bytes withdrawal_credentials,
            bytes amount,
            bytes signature,
            bytes index
        );
    }
);

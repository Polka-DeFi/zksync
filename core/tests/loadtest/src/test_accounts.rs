// Built-in import
use std::sync::atomic::{AtomicU32, Ordering};
// External uses
use num::BigUint;
use rand::Rng;
// Workspace uses
use zksync::{
    error::ClientError, web3::types::H256, EthereumProvider, Network, Provider, Wallet,
    WalletCredentials,
};
use zksync_config::ConfigurationOptions;
use zksync_eth_signer::EthereumSigner;
use zksync_types::{tx::PackedEthSignature, AccountId, Address, ZkSyncTx};

// Local uses
use crate::scenarios::configs::AccountInfo;

#[derive(Debug)]
pub struct TestWallet {
    pub eth_provider: EthereumProvider,
    inner: Wallet,
    nonce: AtomicU32,
}

impl TestWallet {
    pub const TOKEN_NAME: &'static str = "ETH";

    pub async fn from_info(
        info: &AccountInfo,
        provider: Provider,
        config: &ConfigurationOptions,
    ) -> Self {
        let credentials = WalletCredentials::from_eth_signer(
            info.address,
            EthereumSigner::from_key(info.private_key),
            Network::Localhost,
        )
        .await
        .unwrap();

        let inner = Wallet::new(provider, credentials).await.unwrap();
        Self::from_wallet(inner, &config.web3_url).await
    }

    // Parses and builds a new wallets list.
    pub async fn from_info_list(
        input: &[AccountInfo],
        provider: Provider,
        config: &ConfigurationOptions,
    ) -> Vec<Self> {
        let mut wallets = Vec::new();

        for info in input {
            let wallet = Self::from_info(info, provider.clone(), config).await;
            wallets.push(wallet)
        }
        wallets
    }

    // Creates a random wallet.
    pub async fn new_random(provider: Provider, config: &ConfigurationOptions) -> Self {
        let eth_private_key = gen_random_eth_private_key();
        let address_from_pk =
            PackedEthSignature::address_from_private_key(&eth_private_key).unwrap();

        let inner = Wallet::new(
            provider,
            WalletCredentials::from_eth_signer(
                address_from_pk,
                EthereumSigner::from_key(eth_private_key),
                Network::Localhost,
            )
            .await
            .unwrap(),
        )
        .await
        .unwrap();

        Self::from_wallet(inner, &config.web3_url).await
    }

    async fn from_wallet(inner: Wallet, web3_url: impl AsRef<str>) -> Self {
        let eth_provider = inner.ethereum(web3_url).await.unwrap();
        let zk_nonce = inner
            .provider
            .account_info(inner.address())
            .await
            .unwrap()
            .committed
            .nonce;

        Self {
            inner,
            eth_provider,
            nonce: AtomicU32::new(zk_nonce),
        }
    }

    /// Returns the wallet address.
    pub fn address(&self) -> Address {
        self.inner.address()
    }

    /// Returns the current account ID.
    pub fn account_id(&self) -> Option<AccountId> {
        self.inner.account_id()
    }

    // Updates ZKSync account id.
    pub async fn update_account_id(&mut self) -> Result<(), ClientError> {
        self.inner.update_account_id().await
    }

    // Creates a signed change public key transaction.
    pub async fn sign_change_pubkey(&self, fee: BigUint) -> Result<ZkSyncTx, ClientError> {
        self.inner
            .start_change_pubkey()
            .nonce(self.pending_nonce())
            .fee_token(Self::TOKEN_NAME)?
            .fee(fee)
            .tx()
            .await
    }

    // Creates a signed withdraw transaction.
    pub async fn sign_withdraw_single(
        &self,
        amount: BigUint,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        self.inner
            .start_withdraw()
            .nonce(self.pending_nonce())
            .token(Self::TOKEN_NAME)?
            .amount(amount)
            .to(self.inner.address())
            .tx()
            .await
    }

    // Creates a signed withdraw transaction with a fee provided.
    pub async fn sign_withdraw(
        &self,
        amount: BigUint,
        fee: Option<BigUint>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let mut builder = self
            .inner
            .start_withdraw()
            .nonce(self.pending_nonce())
            .token(Self::TOKEN_NAME)?
            .amount(amount)
            .to(self.inner.address());
        if let Some(fee) = fee {
            builder = builder.fee(fee);
        }

        builder.tx().await
    }

    // Creates a signed transfer tx to a given receiver.
    pub async fn sign_transfer(
        &self,
        to: impl Into<Address>,
        amount: impl Into<BigUint>,
        fee: Option<BigUint>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let mut builder = self
            .inner
            .start_transfer()
            .nonce(self.pending_nonce())
            .token(Self::TOKEN_NAME)?
            .amount(amount)
            .to(to.into());
        if let Some(fee) = fee {
            builder = builder.fee(fee);
        }

        builder.tx().await
    }

    // Creates a signed transfer tx to a random receiver.
    pub async fn sign_transfer_to_random(
        &self,
        test_accounts: &[AccountInfo],
        amount: BigUint,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let to = {
            let mut rng = rand::thread_rng();
            let count = test_accounts.len() - 1;

            let mut to_idx = rng.gen_range(0, count);
            while test_accounts[to_idx].address == self.inner.address() {
                to_idx = rng.gen_range(0, count);
            }
            test_accounts[to_idx].address
        };

        self.sign_transfer(to, amount, None).await
    }

    /// Returns appropriate nonce for the new transaction and increments the nonce.
    fn pending_nonce(&self) -> u32 {
        self.nonce.fetch_add(1, Ordering::SeqCst)
    }
}

fn gen_random_eth_private_key() -> H256 {
    let mut eth_private_key = H256::default();
    eth_private_key.randomize();
    eth_private_key
}

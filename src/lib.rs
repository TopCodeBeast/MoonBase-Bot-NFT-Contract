
use std::collections::HashMap;
use std::convert::TryInto;
use std::thread::AccessError;

use near_contract_standards::non_fungible_token::events::{NftMint, NftBurn};
use near_contract_standards::non_fungible_token::metadata::{
    NFTContractMetadata, NonFungibleTokenMetadataProvider, TokenMetadata, NFT_METADATA_SPEC,
};
use near_contract_standards::non_fungible_token::NonFungibleToken;
use near_contract_standards::non_fungible_token::{Token, TokenId};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, UnorderedMap, UnorderedSet, LookupMap, Vector};
use near_sdk::json_types::{U128, U64, Base58CryptoHash};
use near_sdk::serde::{Serialize, Deserialize};
use near_sdk::serde_json::json;
use near_sdk::{
    assert_one_yocto, env, near_bindgen, require, AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseOrValue, Balance, CryptoHash, log, bs58,
};
use utils::verify;

use crate::utils::refund_extra_storage_deposit;

pub mod utils;
pub mod internal;
pub mod view;
pub mod resolver;
pub mod signature;
pub mod owner;

pub type GuildId = String;


#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    collections: UnorderedMap<String, Collection>,
    collections_by_guild_id: UnorderedMap<GuildId, Vec<String>>,
    nft_contracts: UnorderedMap<String, AccountId>,
    public_key: String,
    owner_id: AccountId
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Collection {
    outer_collection_id: String,
    contract_type: String,
    guild_id: String,
	creator_id: AccountId,
    token_metadata: Vector<WrappedTokenMetadata>,
    mintable_roles: Option<Vec<String>>,
    price: u128
}

#[derive(BorshDeserialize, BorshSerialize)]
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct WrappedTokenMetadata {
    metadata: TokenMetadata,
    token_series_id: Option<String>,
    copies: u64,
    minted_count: u64
}


//const PARAS_TOKEN_CONTRACT: &str = "paras-token-v1.testnet";

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Collections,
    CollectionsByGuildId,
    NftContractType
}

#[near_bindgen]
impl Contract {

    #[init]
    pub fn new(public_key: String) -> Self {
        require!(!env::state_exists(), "Already initialized");
        Self {
            collections: UnorderedMap::new(StorageKey::Collections),
            collections_by_guild_id: UnorderedMap::new(StorageKey::CollectionsByGuildId),
            nft_contracts: UnorderedMap::new(StorageKey::NftContractType),
            public_key,
            owner_id: env::predecessor_account_id()
        }
    }

    #[payable]
    pub fn create_collection(&mut self, outer_collection_id: String, contract_type: String, guild_id: String, mintable_roles: Option<Vec<String>>, price: U128, timestamp: U64, sign: String) {
        let initial_storage_usage = env::storage_usage();
        
        let timestamp = u64::from(timestamp);
        assert!(timestamp - env::block_timestamp() < 120_000_000_000, "signature expired");
        let sign: Vec<u8> = bs58::decode(sign).into_vec().unwrap();
        let pk: Vec<u8> = bs58::decode(self.public_key.clone()).into_vec().unwrap();
        verify((env::predecessor_account_id().to_string() + &timestamp.to_string()).into_bytes(), sign.into(), pk.into());
        
        assert!(self.nft_contracts.get(&contract_type).is_some(), "not supported");

        let collection_id = contract_type.clone() + ":" + &outer_collection_id.clone();
        assert!(self.collections.get(&collection_id).is_none(), "already created");
        let collection = Collection {
            outer_collection_id: outer_collection_id.clone(),
            contract_type,
            guild_id: guild_id.clone(),
            creator_id: env::predecessor_account_id(),
            token_metadata: Vector::new([outer_collection_id.clone().as_bytes().to_vec(), "token".as_bytes().to_vec()].concat()),
            mintable_roles,
            price: price.into()
        };
        
        self.collections.insert(&collection_id, &collection);
        let mut collection_ids = self.collections_by_guild_id.get(&guild_id).unwrap_or(Vec::new());
        collection_ids.push(collection_id);
        self.collections_by_guild_id.insert(&guild_id, &collection_ids);
        refund_extra_storage_deposit(env::storage_usage() - initial_storage_usage, 0);
    }

    #[payable]
    pub fn add_token_metadata(&mut self, collection_id: String, token_metadata: TokenMetadata, timestamp: U64, sign: String) {
        let timestamp = u64::from(timestamp);
        assert!(timestamp - env::block_timestamp() < 120_000_000_000, "signature expired");
        let sign: Vec<u8> = bs58::decode(sign).into_vec().unwrap();
        let pk: Vec<u8> = bs58::decode(self.public_key.clone()).into_vec().unwrap();
        verify((env::predecessor_account_id().to_string() + &timestamp.to_string()).into_bytes(), sign.into(), pk.into());

        let collection = self.collections.get(&collection_id).unwrap();
        let contract_id = self.nft_contracts.get(&collection.contract_type).unwrap();
        let token_metadata_json = json!(token_metadata);
        if collection.contract_type == "paras".to_string() {
            Promise::new(contract_id).function_call("nft_create_series".to_string(), json!({
                "token_metadata": token_metadata_json
            }).to_string().into_bytes(), env::attached_deposit() / 2, (env::prepaid_gas() - env::used_gas()) / 3).then(
                Promise::new(env::current_account_id()).function_call("on_add_token_metadata".to_string(), json!({
                    "collection_id": collection_id,
                    "token_metadata": token_metadata_json
                }).to_string().into_bytes(), env::attached_deposit() / 2, (env::prepaid_gas() - env::used_gas()) / 3)
            );
        }
    }

    #[payable]
    pub fn set_mintable_roles(&mut self, collection_id: String, mintable_roles: Option<Vec<String>>, timestamp: U64, sign: String) {
        let initial_storage_usage = env::storage_usage();

        let timestamp = u64::from(timestamp);
        assert!(timestamp - env::block_timestamp() < 120_000_000_000, "signature expired");
        let sign: Vec<u8> = bs58::decode(sign).into_vec().unwrap();
        let pk: Vec<u8> = bs58::decode(self.public_key.clone()).into_vec().unwrap();
        verify((env::predecessor_account_id().to_string() + &timestamp.to_string()).into_bytes(), sign.into(), pk.into());

        let mut collection = self.collections.get(&collection_id).unwrap();
        collection.mintable_roles = mintable_roles;
        self.collections.insert(&collection_id, &collection);
        refund_extra_storage_deposit(env::storage_usage() - initial_storage_usage, 0);
    }

    pub fn set_price(&mut self, collection_id: String, price: U128, timestamp: U64, sign: String) {
        let timestamp = u64::from(timestamp);
        assert!(timestamp - env::block_timestamp() < 120_000_000_000, "signature expired");
        let sign: Vec<u8> = bs58::decode(sign).into_vec().unwrap();
        let pk: Vec<u8> = bs58::decode(self.public_key.clone()).into_vec().unwrap();
        verify((env::predecessor_account_id().to_string() + &timestamp.to_string()).into_bytes(), sign.into(), pk.into());

        let mut collection = self.collections.get(&collection_id).unwrap();
        collection.price = price.into();
        self.collections.insert(&collection_id, &collection);
    }

    #[payable]
    pub fn nft_mint(&mut self, collection_id: String, timestamp: U64, sign: String) {

        let timestamp = u64::from(timestamp);
        assert!(timestamp - env::block_timestamp() < 120_000_000_000, "signature expired");
        let sign: Vec<u8> = bs58::decode(sign).into_vec().unwrap();
        let pk: Vec<u8> = bs58::decode(self.public_key.clone()).into_vec().unwrap();
        verify((env::predecessor_account_id().to_string() + &collection_id).into_bytes(), sign.into(), pk.into());

        let sender_id = env::predecessor_account_id();
        let collection = self.collections.get(&collection_id).unwrap();
        assert!(collection.price < env::attached_deposit(), "not enough balance");
        let nft_contract_id = self.nft_contracts.get(&collection.contract_type).unwrap();
        let mut total_copies: u64 = 0;
        collection.token_metadata.iter().for_each(|item| {
            total_copies += item.copies - item.minted_count;
        });
        let mut random_index = u64::try_from_slice(&env::keccak512(&env::random_seed())[0..8]).unwrap() / total_copies;
        let mut token_metadata_index: u64 = 0;
        for (i, item) in collection.token_metadata.iter().enumerate() {
            if random_index < item.copies - item.minted_count {
                token_metadata_index = i as u64;
            }
            random_index -= item.copies - item.minted_count;
        }
        let token_metadata = collection.token_metadata.get(token_metadata_index).unwrap();
        let promise = Promise::new(nft_contract_id);
        if collection.contract_type == "paras".to_string() {
            promise.function_call("nft_mint".to_string(), json!({
                "token_series_id": token_metadata.token_series_id.unwrap(),
                "receiver_id": sender_id 
            }).to_string().into_bytes(), env::attached_deposit() - collection.price, (env::prepaid_gas() - env::used_gas()) / 3).then(
                Promise::new(env::current_account_id()).function_call("on_nft_mint".to_string(), json!({
                    "collection_id": collection_id,
                    "token_metadata_index": U64::from(token_metadata_index)
                }).to_string().into_bytes(), collection.price, (env::prepaid_gas() - env::used_gas()) / 3)
            );
        }
    }

}

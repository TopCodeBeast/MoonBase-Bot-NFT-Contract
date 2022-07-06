
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
use near_sdk::serde_json::{json, self};
use near_sdk::{
    assert_one_yocto, env, near_bindgen, require, AccountId, BorshStorageKey, PanicOnDefault, Promise, PromiseOrValue, Balance, CryptoHash, log, bs58,
};
use utils::verify;

use crate::utils::refund_extra_storage_deposit;

pub mod utils;
pub mod internal;
pub mod view;
pub mod resolver;
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
pub struct OldCollection {
    outer_collection_id: String,
    contract_type: String,
    guild_id: String,
	creator_id: AccountId,
    token_metadata: Vector<WrappedTokenMetadata>,
    mintable_roles: Option<Vec<String>>,
    price: u128,
    royalty: Option<HashMap<AccountId, u32>>,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Collection {
    outer_collection_id: String,
    contract_type: String,
    guild_id: String,
	creator_id: AccountId,
    token_metadata: Vector<WrappedTokenMetadata>,
    mintable_roles: Option<Vec<String>>,
    price: u128,
    royalty: Option<HashMap<AccountId, u32>>,
    mint_count_limit: Option<u32>
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

    pub fn fix(&mut self) {
        assert!(self.owner_id == env::predecessor_account_id(), "owner only");
        let keys = self.collections.keys_as_vector().to_vec();
        for key in keys {
            let key_raw = key.try_to_vec().unwrap();
            let old_collection = OldCollection::try_from_slice(&self.collections.remove_raw(&key_raw).unwrap()).unwrap();
            self.collections.insert(&key, &Collection { 
                outer_collection_id: old_collection.outer_collection_id,
                contract_type: old_collection.contract_type,
                guild_id: old_collection.guild_id,
                creator_id: old_collection.creator_id,
                token_metadata: old_collection.token_metadata,
                mintable_roles: old_collection.mintable_roles,
                price: old_collection.price,
                royalty: old_collection.royalty,
                mint_count_limit: None
            });
        }
    }

    #[payable]
    pub fn create_collection(&mut self, outer_collection_id: String, contract_type: String, guild_id: String, mintable_roles: Option<Vec<String>>, price: U128, royalty: Option<HashMap<AccountId, u32>>, mint_count_limit: Option<u32>, timestamp: U64, sign: String) {
        let initial_storage_usage = env::storage_usage();
        
        let timestamp = u64::from(timestamp);
        assert!(env::block_timestamp() - timestamp < 120_000_000_000, "signature expired");
        let sign: Vec<u8> = bs58::decode(sign).into_vec().unwrap();
        let pk: Vec<u8> = bs58::decode(self.public_key.clone()).into_vec().unwrap();
        let json = json!(env::predecessor_account_id().to_string() + &timestamp.to_string()).to_string();
        verify(json.into_bytes(), sign.into(), pk.into());
        
        assert!(self.nft_contracts.get(&contract_type).is_some(), "not supported");

        let mut total_perpetual = 0;
        let mut total_accounts = 0;
        if let Some(royalty) = royalty.clone() {
            for (_ , v) in royalty.clone().iter() {
                total_perpetual += *v;
                total_accounts += 1;
            }
        }
        assert!(total_accounts <= 10, "royalty exceeds 10 accounts");
        assert!(
            total_perpetual <= 9000,
            "Exceeds maximum royalty -> 9000",
        );

        let collection_id = contract_type.clone() + ":" + &outer_collection_id.clone();
        assert!(self.collections.get(&collection_id).is_none(), "already created");
        let collection = Collection {
            outer_collection_id: outer_collection_id.clone(),
            contract_type,
            guild_id: guild_id.clone(),
            creator_id: env::predecessor_account_id(),
            token_metadata: Vector::new([outer_collection_id.clone().as_bytes().to_vec(), "token".as_bytes().to_vec()].concat()),
            mintable_roles,
            price: price.into(),
            royalty,
            mint_count_limit
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
        assert!(env::block_timestamp() - timestamp < 120_000_000_000, "signature expired");
        let sign: Vec<u8> = bs58::decode(sign).into_vec().unwrap();
        let pk: Vec<u8> = bs58::decode(self.public_key.clone()).into_vec().unwrap();
        let json = json!(env::predecessor_account_id().to_string() + &timestamp.to_string()).to_string();
        verify(json.into_bytes(), sign.into(), pk.into());

        let collection = self.collections.get(&collection_id).unwrap();
        let contract_id = self.nft_contracts.get(&collection.contract_type).unwrap();
        let token_metadata_json = json!(token_metadata);
        let royalty_json = json!(collection.royalty);
        if collection.contract_type == "paras".to_string() {
            Promise::new(contract_id).function_call("nft_create_series".to_string(), json!({
                "token_metadata": token_metadata_json,
                "royalty": royalty_json
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
        assert!(env::block_timestamp() - timestamp < 120_000_000_000, "signature expired");
        let sign: Vec<u8> = bs58::decode(sign).into_vec().unwrap();
        let pk: Vec<u8> = bs58::decode(self.public_key.clone()).into_vec().unwrap();
        let json = json!(env::predecessor_account_id().to_string() + &timestamp.to_string()).to_string();
        verify(json.into_bytes(), sign.into(), pk.into());

        let mut collection = self.collections.get(&collection_id).unwrap();
        collection.mintable_roles = mintable_roles;
        self.collections.insert(&collection_id, &collection);
        refund_extra_storage_deposit(env::storage_usage() - initial_storage_usage, 0);
    }

    pub fn set_price(&mut self, collection_id: String, price: U128, timestamp: U64, sign: String) {
        let timestamp = u64::from(timestamp);
        assert!(env::block_timestamp() - timestamp < 120_000_000_000, "signature expired");
        let sign: Vec<u8> = bs58::decode(sign).into_vec().unwrap();
        let pk: Vec<u8> = bs58::decode(self.public_key.clone()).into_vec().unwrap();
        let json = json!(env::predecessor_account_id().to_string() + &timestamp.to_string()).to_string();
        verify(json.into_bytes(), sign.into(), pk.into());

        let mut collection = self.collections.get(&collection_id).unwrap();
        collection.price = price.into();
        self.collections.insert(&collection_id, &collection);
    }

    #[payable]
    pub fn nft_mint(&mut self, collection_id: String, timestamp: U64, sign: String) {

        let timestamp = u64::from(timestamp);
        assert!(env::block_timestamp() - timestamp < 120_000_000_000, "signature expired");
        let sign: Vec<u8> = bs58::decode(sign).into_vec().unwrap();
        let pk: Vec<u8> = bs58::decode(self.public_key.clone()).into_vec().unwrap();
        let json = json!(env::predecessor_account_id().to_string() + &timestamp.to_string() + &collection_id).to_string();
        verify(json.into_bytes(), sign.into(), pk.into());

        let sender_id = env::predecessor_account_id();
        let collection = self.collections.get(&collection_id).unwrap();
        assert!(collection.price < env::attached_deposit(), "not enough balance");
        let nft_contract_id = self.nft_contracts.get(&collection.contract_type).unwrap();
        let mut total_copies = 0;
        let mut total_minted = 0;
        collection.token_metadata.iter().for_each(|item| {
            total_copies += item.copies - item.minted_count;
            total_minted += item.minted_count;
        });

        assert!(collection.mint_count_limit.is_none() || collection.mint_count_limit.unwrap() > total_minted as u32, "exceed mint count limit");

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

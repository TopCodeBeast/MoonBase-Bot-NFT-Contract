use crate::*;

#[derive(BorshDeserialize, BorshSerialize)]
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct CollectionInfo {
    collection_id: String,
    outer_collection_id: String,
    contract_type: String,
    guild_id: String,
	creator_id: AccountId,
    mintable_roles: Option<Vec<String>>,
    royalty: Option<HashMap<AccountId, u32>>,
    price: U128,
    mint_count_limit: Option<u32>
}


#[near_bindgen]
impl Contract {
    pub fn get_collection(&self, collection_id: String) -> CollectionInfo {
        let collection = self.collections.get(&collection_id).unwrap();
        CollectionInfo { 
            collection_id,
            outer_collection_id: collection.outer_collection_id,
            contract_type: collection.contract_type, 
            guild_id: collection.guild_id, 
            creator_id: collection.creator_id, 
            mintable_roles: collection.mintable_roles, 
            royalty: collection.royalty,
            price: collection.price.into(),
            mint_count_limit: collection.mint_count_limit
        }
    }

    pub fn get_token_metadata(&self, collection_id: String) -> Vec<WrappedTokenMetadata> {
        self.collections.get(&collection_id).unwrap().token_metadata.to_vec()
    }

    pub fn get_collections_by_guild(&self, guild_id: String) -> Vec<CollectionInfo> {
        let collection_ids = self.collections_by_guild_id.get(&guild_id).unwrap();
        collection_ids.iter().map(|collection_id| {
            let collection = self.collections.get(&collection_id).unwrap();
            CollectionInfo { 
                collection_id: collection_id.clone(),
                outer_collection_id: collection.outer_collection_id,
                contract_type: collection.contract_type, 
                guild_id: collection.guild_id, 
                creator_id: collection.creator_id, 
                mintable_roles: collection.mintable_roles, 
                royalty: collection.royalty,
                price: collection.price.into(), 
                mint_count_limit: collection.mint_count_limit
            }
        })
        .collect()
    }

    pub fn get_minted_count_by_collection(&self, collection_id: String) -> U64 {
        let collection = self.collections.get(&collection_id).unwrap();
        let mut total_minted = 0;
        collection.token_metadata.iter().for_each(|item| {
            total_minted += item.minted_count;
        });
        total_minted.into()
    }
}

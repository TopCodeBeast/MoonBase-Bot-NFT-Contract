

use near_sdk::serde_json;

use crate::*;

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ParasMessage {
    token_series_id: String,
	metadata: TokenMetadata,
	creator_id: AccountId,
    royalty: HashMap<AccountId, u32>,
    transaction_fee: Option<U128>
}

impl Contract {
    pub(crate) fn internal_add_token_metadata(&mut self, collection_id: String, token_metadata: TokenMetadata, extra: Option<String>) {
        let initial_storage_usage = env::storage_usage();
        let copies = match token_metadata.copies {
            Some(v) => u64::from(v),
            None => 1 as u64
        };
        let mut collection = self.collections.get(&collection_id).unwrap();
        let mut token_series_id = None;
        if collection.contract_type == "paras".to_string() {
            let extra = serde_json::from_str::<ParasMessage>(&extra.unwrap()).unwrap();
            token_series_id = Some(extra.token_series_id.clone());
        }
        collection.token_metadata.push(&WrappedTokenMetadata { metadata: token_metadata, token_series_id, copies, minted_count: 0 });
        self.collections.insert(&collection_id, &collection);
        refund_extra_storage_deposit(env::storage_usage() - initial_storage_usage, 0);
    }
}


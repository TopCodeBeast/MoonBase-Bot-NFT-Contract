use near_sdk::PromiseResult;

use crate::*;

#[near_bindgen]
impl Contract {
    #[private]
    #[payable]
    pub fn on_add_token_metadata(&mut self, collection_id: String, token_metadata: TokenMetadata) {
        match env::promise_result(0) {
            PromiseResult::NotReady => {
                panic!("not ready")
            }
            PromiseResult::Successful(data) => {
                self.internal_add_token_metadata(collection_id, token_metadata, Some(String::from_utf8(data).unwrap()));
            }
            PromiseResult::Failed => {
                refund_extra_storage_deposit(0, 0);
            }
        }
    }

    #[private]
    #[payable]
    pub fn on_nft_mint(&mut self, collection_id: String, token_metadata_index: U64) {
        match env::promise_result(0) {
            PromiseResult::NotReady => {
                panic!("not ready")
            }
            PromiseResult::Successful(_) => {
                let mut collection = self.collections.get(&collection_id).unwrap();
                let mut metadata = collection.token_metadata.get(token_metadata_index.into()).unwrap();
                metadata.minted_count += 1;
                collection.token_metadata.replace(token_metadata_index.into(), &metadata);
                self.collections.insert(&collection_id, &collection);
                Promise::new(collection.creator_id).transfer(collection.price);
                refund_extra_storage_deposit(0, collection.price);
            }
            PromiseResult::Failed => {
                refund_extra_storage_deposit(0, 0);
            }
        }
    }
}
use crate::*;

#[near_bindgen]
impl Contract {
    pub fn set_contract_type(&mut self, contract_type: String, contract_id: AccountId) {
        assert!(self.owner_id == env::predecessor_account_id(), "owner only");

        self.nft_contracts.insert(&contract_type, &contract_id);
    }

    pub fn del_contract_type(&mut self, contract_type: String) {
        assert!(self.owner_id == env::predecessor_account_id(), "owner only");

        self.nft_contracts.remove(&contract_type);
    }
}
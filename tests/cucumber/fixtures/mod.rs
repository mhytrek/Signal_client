use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    pub account_name: String,
    pub uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_name_in_bob: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_name_in_alice: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestConfig {
    pub accounts: HashMap<String, AccountConfig>,
}

impl TestConfig {
    pub fn load() -> Result<Self> {
        let content = fs::read_to_string("tests/fixtures/test_config.json")?;
        let config: TestConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn get_account(&self, alias: &str) -> Option<&AccountConfig> {
        self.accounts.get(alias)
    }

    pub fn get_contact_name(&self, owner: &str, contact_alias: &str) -> Option<String> {
        match (owner, contact_alias) {
            ("alice", "bob") => self.accounts.get("bob")?.contact_name_in_alice.clone(),
            ("bob", "alice") => self.accounts.get("alice")?.contact_name_in_bob.clone(),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct TestAccount {
    pub name: String,
}

impl TestAccount {
    pub async fn load(alias: &str) -> Result<Self> {
        let config = TestConfig::load()?;
        let account_config = config
            .get_account(alias)
            .ok_or_else(|| anyhow::anyhow!("Account {alias} not found in test config"))?;

        Ok(Self {
            name: account_config.account_name.clone(),
        })
    }
}

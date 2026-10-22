use ledgercraft_core::Money;
use serde::{Deserialize, Serialize};

use crate::{AccountId, ItemId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    #[default]
    Service,
    NonInventory,
    OtherCharge,
}

impl ItemKind {
    pub const ALL: [Self; 3] = [Self::Service, Self::NonInventory, Self::OtherCharge];

    pub fn label(self) -> &'static str {
        match self {
            Self::Service => "Service",
            Self::NonInventory => "Non-inventory Part",
            Self::OtherCharge => "Other Charge",
        }
    }
}

/// What a user fills in to create or edit an item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemSpec {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub kind: ItemKind,
    #[serde(default)]
    pub rate: Money,
    pub income_account: AccountId,
    /// Whether sales tax is charged on it. Items in files made before sales
    /// tax count as taxable.
    #[serde(default = "crate::txn::taxable_by_default")]
    pub taxable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub spec: ItemSpec,
    pub active: bool,
}

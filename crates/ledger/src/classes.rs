//! Classes: a label on transactions, such as a department, a location or a
//! line of business, to see profit for each (Reports > Profit & Loss by
//! Class). A transaction has at most one class.

use serde::{Deserialize, Serialize};

use crate::{Company, LedgerError, LedgerResult, TxnClassId, TxnId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxnClass {
    pub id: TxnClassId,
    pub name: String,
    #[serde(default = "yes")]
    pub active: bool,
}

fn yes() -> bool {
    true
}

impl Company {
    pub fn class(&self, id: TxnClassId) -> Option<&TxnClass> {
        self.classes.iter().find(|c| c.id == id)
    }

    /// A class's name, or its id if it is gone.
    pub fn class_name(&self, id: TxnClassId) -> String {
        self.class(id)
            .map_or_else(|| id.to_string(), |c| c.name.clone())
    }

    fn check_class_name(&self, name: &str, me: Option<TxnClassId>) -> LedgerResult<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(LedgerError::Required("A class name"));
        }
        if self
            .classes
            .iter()
            .any(|c| Some(c.id) != me && c.name.eq_ignore_ascii_case(name))
        {
            return Err(LedgerError::Duplicate(format!("A class named {name:?}")));
        }
        Ok(name.to_string())
    }

    pub fn add_class(&mut self, name: &str) -> LedgerResult<TxnClassId> {
        let name = self.check_class_name(name, None)?;
        let id = TxnClassId(self.alloc());
        self.classes.push(TxnClass {
            id,
            name,
            active: true,
        });
        Ok(id)
    }

    pub fn rename_class(&mut self, id: TxnClassId, name: &str) -> LedgerResult<()> {
        let name = self.check_class_name(name, Some(id))?;
        let class = self
            .classes
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Class {id}")))?;
        class.name = name;
        Ok(())
    }

    /// An inactive class is left off the forms; transactions keep it.
    pub fn set_class_active(&mut self, id: TxnClassId, active: bool) -> LedgerResult<()> {
        let class = self
            .classes
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| LedgerError::NotFound(format!("Class {id}")))?;
        class.active = active;
        Ok(())
    }

    /// Puts transaction `txn` in `class`, or in no class with `None`.
    pub fn set_txn_class(&mut self, txn: TxnId, class: Option<TxnClassId>) -> LedgerResult<()> {
        if let Some(id) = class {
            self.class(id)
                .ok_or_else(|| LedgerError::NotFound(format!("Class {id}")))?;
        }
        let t = self
            .transactions
            .iter_mut()
            .find(|t| t.id == txn)
            .ok_or_else(|| LedgerError::NotFound(format!("Transaction {txn}")))?;
        t.class = class;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_are_named_once_and_set_on_transactions() {
        let mut c = Company::with_default_accounts("Class Co");
        let shop = c.add_class("Shop").unwrap();
        assert!(c.add_class(" shop ").is_err(), "names are unique");
        assert!(c.add_class("  ").is_err());
        let fairs = c.add_class("Craft fairs").unwrap();
        c.rename_class(fairs, "Fairs").unwrap();
        assert_eq!(c.class_name(fairs), "Fairs");
        assert!(c.rename_class(fairs, "Shop").is_err());
        c.set_class_active(shop, false).unwrap();
        assert!(!c.class(shop).unwrap().active);
        assert!(c.set_txn_class(TxnId(999), Some(shop)).is_err());
    }
}

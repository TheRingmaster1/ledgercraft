use serde::{Deserialize, Serialize};

use crate::{CustomerId, VendorId};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Address {
    pub line1: String,
    pub line2: String,
    pub city: String,
    pub region: String,
    pub postal_code: String,
    pub country: String,
}

impl Address {
    pub fn is_empty(&self) -> bool {
        self.line1.is_empty()
            && self.line2.is_empty()
            && self.city.is_empty()
            && self.region.is_empty()
            && self.postal_code.is_empty()
    }

    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for l in [&self.line1, &self.line2] {
            if !l.is_empty() {
                out.push(l.clone());
            }
        }
        let city_line = [
            self.city.as_str(),
            self.region.as_str(),
            self.postal_code.as_str(),
        ]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ");
        if !city_line.is_empty() {
            out.push(city_line);
        }
        if !self.country.is_empty() {
            out.push(self.country.clone());
        }
        out
    }
}

/// The details a customer and a vendor share.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Contact {
    pub name: String,
    pub company: String,
    pub email: String,
    pub phone: String,
    pub address: Address,
    /// Payment terms in days (Net 30 = 30). Zero means due on receipt.
    pub terms_days: u32,
    pub notes: String,
    /// Where goods go, when not to `address` (customers only).
    #[serde(default, skip_serializing_if = "Address::is_empty")]
    pub ship_to: Address,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Customer {
    pub id: CustomerId,
    pub contact: Contact,
    pub active: bool,
    /// Their tax group and any exemption.
    #[serde(default)]
    pub tax: crate::CustomerTax,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vendor {
    pub id: VendorId,
    pub contact: Contact,
    pub active: bool,
}

/// The name on a transaction line: a customer or a vendor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Party {
    Customer(CustomerId),
    Vendor(VendorId),
}

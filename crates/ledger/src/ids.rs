use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident, $prefix:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", $prefix, self.0)
            }
        }
    };
}

define_id!(
    /// Identifies an account in the chart of accounts.
    AccountId, "A"
);
define_id!(
    /// Identifies a customer.
    CustomerId, "C"
);
define_id!(
    /// Identifies a vendor.
    VendorId, "V"
);
define_id!(
    /// Identifies a product or service item.
    ItemId, "I"
);
define_id!(
    /// Identifies a transaction.
    TxnId, "T"
);
define_id!(
    /// Identifies one sales tax component: a rate charged for one agency,
    /// such as a state, county, city or transit district.
    TaxComponentId, "X"
);
define_id!(
    /// Identifies a sales tax group: the components charged together at a
    /// location (state + county + city ...), chosen on invoices.
    TaxGroupId, "G"
);
define_id!(
    /// Identifies a class (a department, location or line of business).
    TxnClassId, "K"
);
define_id!(
    /// Identifies a recurring (memorized) transaction.
    RecurringId, "M"
);

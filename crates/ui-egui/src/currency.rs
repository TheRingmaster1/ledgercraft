//! The currency amounts are shown in, chosen in Help > Preferences.
//!
//! This changes how amounts look (`$1,234.56`, `€1,234.56`), not their value:
//! LedgerCraft does not convert between currencies. Every amount on screen
//! goes through [`format`], so the whole app follows the setting at once.

use std::sync::atomic::{AtomicUsize, Ordering};

use ledgercraft_core::Money;
use serde::{Deserialize, Serialize};

/// The currencies offered. Their symbols are all in egui's built-in font, so
/// none shows as an empty box; others use their code instead of a symbol.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum Currency {
    #[default]
    Usd,
    Cad,
    Aud,
    Nzd,
    Mxn,
    Eur,
    Gbp,
    Chf,
    Zar,
    Sek,
}

impl Currency {
    pub(crate) const ALL: [Self; 10] = [
        Self::Usd,
        Self::Cad,
        Self::Aud,
        Self::Nzd,
        Self::Mxn,
        Self::Eur,
        Self::Gbp,
        Self::Chf,
        Self::Zar,
        Self::Sek,
    ];

    /// What goes in front of an amount.
    pub(crate) fn symbol(self) -> &'static str {
        match self {
            Self::Usd | Self::Cad | Self::Aud | Self::Nzd | Self::Mxn => "$",
            Self::Eur => "€",
            Self::Gbp => "£",
            Self::Chf => "CHF ",
            Self::Zar => "R ",
            Self::Sek => "SEK ",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Usd => "US dollar (USD, $)",
            Self::Cad => "Canadian dollar (CAD, $)",
            Self::Aud => "Australian dollar (AUD, $)",
            Self::Nzd => "New Zealand dollar (NZD, $)",
            Self::Mxn => "Mexican peso (MXN, $)",
            Self::Eur => "Euro (EUR, €)",
            Self::Gbp => "British pound (GBP, £)",
            Self::Chf => "Swiss franc (CHF)",
            Self::Zar => "South African rand (ZAR, R)",
            Self::Sek => "Swedish krona (SEK)",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }
}

/// The currency in use, as an index into [`Currency::ALL`]. A plain atomic
/// rather than state threaded through every page: it is set from the
/// preferences once per frame and read wherever an amount is drawn.
static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn set(currency: Currency) {
    CURRENT.store(currency.index(), Ordering::Relaxed);
}

pub(crate) fn current() -> Currency {
    Currency::ALL
        .get(CURRENT.load(Ordering::Relaxed))
        .copied()
        .unwrap_or_default()
}

/// An amount as the app shows it: `$1,234.56`, `-€89.10`.
pub(crate) fn format(m: Money) -> String {
    m.format_with(current().symbol())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_currency_formats_and_round_trips() {
        for c in Currency::ALL {
            let m = Money::from_cents(-123_456);
            let shown = m.format_with(c.symbol());
            assert_eq!(Money::parse(&shown), Ok(m), "{c:?} shows {shown}");
            assert_eq!(Currency::ALL.get(c.index()), Some(&c));
        }
    }

    #[test]
    fn saved_as_the_currency_code() {
        assert_eq!(serde_json::to_string(&Currency::Eur).unwrap(), "\"EUR\"");
        assert_eq!(
            serde_json::from_str::<Currency>("\"GBP\"").unwrap(),
            Currency::Gbp
        );
    }
}

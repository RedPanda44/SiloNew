//! The Silo interface layer: Apple Wallet-inspired theme, icons,
//! widgets, and the five page views.

pub mod dashboard;
pub mod help;
pub mod icons;
pub mod passcode;
pub mod settings;
pub mod theme;
pub mod wallet;
pub mod widgets;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Dashboard,
    Wallet,
    Passcode,
    Settings,
    Help,
}

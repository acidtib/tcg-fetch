use clap::ValueEnum;

#[derive(Debug, Clone, ValueEnum)]
pub enum TcgType {
    /// Magic: The Gathering
    Mtg,
    /// Grand Archive
    Ga,
}

// Unified card structure for both MTG and GA
#[derive(Debug, Clone)]
pub struct UnifiedCard {
    pub id: String,
    pub image_url: String,
}

// Re-export TCG-specific modules
pub mod ga;
pub mod mtg;

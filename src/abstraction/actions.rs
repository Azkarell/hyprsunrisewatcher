use std::{fmt::Display, path::PathBuf};

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Encode, Decode, PartialEq, Eq, Clone)]
pub enum Action {
    Stop,
    Enable,
    Disable,
    Toggle,
    ReloadConfig,
    Trigger { action: String },
    Nothing,
    GenerateDefaultConfig { path: PathBuf },
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Stop => f.write_str("Action - Stop"),
            Action::Enable => f.write_str("Action - Enable"),
            Action::Disable => f.write_str("Action - Disable"),
            Action::Toggle => f.write_str("Action - Toggle"),
            Action::ReloadConfig => f.write_str(&format!("Action - Relod ")),
            Action::Trigger { action } => f.write_str(&format!("Action - Trigger - {}", action)),
            Action::Nothing => f.write_str("Action -Nothing"),
        }
    }
}

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use type4we_core::hotkey::ChordTracker;
use type4we_core::secrets::SecretStore;
use type4we_core::Config;

use crate::messages::CoordCommand;

#[derive(Clone)]
pub struct SharedState {
    pub config: Arc<RwLock<Config>>,
    pub config_path: PathBuf,
    pub tracker: Arc<Mutex<ChordTracker>>,
    pub secrets: Arc<dyn SecretStore>,
    pub commands: tokio::sync::mpsc::UnboundedSender<CoordCommand>,
}

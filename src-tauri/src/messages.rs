use type4we_core::asr::EngineEvent;
use type4we_core::hotkey::Edge;

#[derive(Debug, Clone)]
pub enum CoordCommand {
    Hotkey(Edge),
    Engine { id: u64, event: EngineEvent },
}

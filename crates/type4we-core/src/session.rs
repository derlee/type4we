//! Dictation session state machine.
//!
//! The machine is synchronous and side-effect free. The app coordinator applies
//! the returned [`Effect`]s: opening the microphone and ASR stream, asking the
//! stream to finish, and injecting text. Engine events carry the session id
//! assigned at start so a late message from a previous session is ignored.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseKind {
    Idle,
    Starting,
    Listening,
    Finalizing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Starting { id: u64 },
    Listening { id: u64 },
    Finalizing { id: u64 },
}

impl Phase {
    pub fn kind(&self) -> PhaseKind {
        match self {
            Self::Idle => PhaseKind::Idle,
            Self::Starting { .. } => PhaseKind::Starting,
            Self::Listening { .. } => PhaseKind::Listening,
            Self::Finalizing { .. } => PhaseKind::Finalizing,
        }
    }

    pub fn id(&self) -> Option<u64> {
        match self {
            Self::Idle => None,
            Self::Starting { id } | Self::Listening { id } | Self::Finalizing { id } => Some(*id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// Hotkey asked to begin dictation.
    Start,
    /// Hotkey asked to end dictation.
    Stop,
    EngineReady {
        id: u64,
    },
    EngineFailed {
        id: u64,
        message: String,
    },
    /// Newly finalized text that should be injected now. Empty strings are ignored.
    Transcript {
        id: u64,
        newly_final: String,
    },
    /// The recognizer finished. `remainder` is trailing non-final text.
    Finished {
        id: u64,
        remainder: String,
    },
    InjectFailed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    BeginSession { id: u64 },
    EndSession { id: u64 },
    Inject(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub phase: Phase,
    next_id: u64,
    pub last_error: Option<String>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            next_id: 1,
            last_error: None,
        }
    }

    pub fn handle(&mut self, event: SessionEvent) -> Vec<Effect> {
        match event {
            SessionEvent::Start => self.start(),
            SessionEvent::Stop => self.stop(),
            SessionEvent::EngineReady { id } => self.engine_ready(id),
            SessionEvent::EngineFailed { id, message } => self.fail(id, message),
            SessionEvent::Transcript { id, newly_final } => self.transcript(id, newly_final),
            SessionEvent::Finished { id, remainder } => self.finished(id, remainder),
            SessionEvent::InjectFailed { message } => self.inject_failed(message),
        }
    }

    fn start(&mut self) -> Vec<Effect> {
        if !matches!(self.phase, Phase::Idle) {
            return Vec::new();
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.phase = Phase::Starting { id };
        self.last_error = None;
        vec![Effect::BeginSession { id }]
    }

    fn stop(&mut self) -> Vec<Effect> {
        match self.phase {
            Phase::Starting { id } | Phase::Listening { id } => {
                self.phase = Phase::Finalizing { id };
                vec![Effect::EndSession { id }]
            }
            Phase::Idle | Phase::Finalizing { .. } => Vec::new(),
        }
    }

    fn engine_ready(&mut self, id: u64) -> Vec<Effect> {
        if self.phase == (Phase::Starting { id }) {
            self.phase = Phase::Listening { id };
        }
        Vec::new()
    }

    fn transcript(&mut self, id: u64, newly_final: String) -> Vec<Effect> {
        if !self.matches(id) || newly_final.is_empty() {
            return Vec::new();
        }
        if matches!(
            self.phase,
            Phase::Listening { .. } | Phase::Finalizing { .. }
        ) {
            vec![Effect::Inject(newly_final)]
        } else {
            Vec::new()
        }
    }

    fn finished(&mut self, id: u64, remainder: String) -> Vec<Effect> {
        if !self.matches(id) {
            return Vec::new();
        }
        if !matches!(
            self.phase,
            Phase::Listening { .. } | Phase::Finalizing { .. } | Phase::Starting { .. }
        ) {
            return Vec::new();
        }
        self.phase = Phase::Idle;
        if remainder.is_empty() {
            Vec::new()
        } else {
            vec![Effect::Inject(remainder)]
        }
    }

    fn fail(&mut self, id: u64, message: String) -> Vec<Effect> {
        if !self.matches(id) {
            return Vec::new();
        }
        let end = self.phase.id().map(|id| Effect::EndSession { id });
        self.phase = Phase::Idle;
        self.last_error = Some(message);
        end.into_iter().collect()
    }

    fn inject_failed(&mut self, message: String) -> Vec<Effect> {
        let end = self.phase.id().map(|id| Effect::EndSession { id });
        self.phase = Phase::Idle;
        self.last_error = Some(message);
        end.into_iter().collect()
    }

    fn matches(&self, id: u64) -> bool {
        self.phase.id() == Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(session: &mut Session) -> u64 {
        let effects = session.handle(SessionEvent::Start);
        match effects.as_slice() {
            [Effect::BeginSession { id }] => *id,
            other => panic!("expected BeginSession, got {other:?}"),
        }
    }

    #[test]
    fn push_to_talk_injects_finals_then_remainder() {
        let mut session = Session::new();
        let id = start(&mut session);
        assert_eq!(session.phase, Phase::Starting { id });

        assert!(session.handle(SessionEvent::EngineReady { id }).is_empty());
        assert_eq!(session.phase, Phase::Listening { id });

        let effects = session.handle(SessionEvent::Transcript {
            id,
            newly_final: "Hello ".into(),
        });
        assert_eq!(effects, vec![Effect::Inject("Hello ".into())]);
        assert_eq!(session.phase, Phase::Listening { id });

        assert!(session
            .handle(SessionEvent::Transcript {
                id,
                newly_final: String::new(),
            })
            .is_empty());

        let effects = session.handle(SessionEvent::Stop);
        assert_eq!(effects, vec![Effect::EndSession { id }]);
        assert_eq!(session.phase, Phase::Finalizing { id });

        let effects = session.handle(SessionEvent::Finished {
            id,
            remainder: "world".into(),
        });
        assert_eq!(effects, vec![Effect::Inject("world".into())]);
        assert_eq!(session.phase, Phase::Idle);
        assert!(session.last_error.is_none());
    }

    #[test]
    fn empty_result_does_not_inject() {
        let mut session = Session::new();
        let id = start(&mut session);
        session.handle(SessionEvent::EngineReady { id });
        session.handle(SessionEvent::Stop);
        let effects = session.handle(SessionEvent::Finished {
            id,
            remainder: String::new(),
        });
        assert!(effects.is_empty());
        assert_eq!(session.phase, Phase::Idle);
    }

    #[test]
    fn second_start_while_listening_is_ignored() {
        let mut session = Session::new();
        let id = start(&mut session);
        session.handle(SessionEvent::EngineReady { id });
        assert!(session.handle(SessionEvent::Start).is_empty());
        assert_eq!(session.phase, Phase::Listening { id });
    }

    #[test]
    fn stop_while_idle_is_ignored() {
        let mut session = Session::new();
        assert!(session.handle(SessionEvent::Stop).is_empty());
        assert_eq!(session.phase, Phase::Idle);
    }

    #[test]
    fn release_before_engine_is_ready_still_finishes() {
        let mut session = Session::new();
        let id = start(&mut session);
        let effects = session.handle(SessionEvent::Stop);
        assert_eq!(effects, vec![Effect::EndSession { id }]);
        assert_eq!(session.phase, Phase::Finalizing { id });
        let effects = session.handle(SessionEvent::Finished {
            id,
            remainder: "late".into(),
        });
        assert_eq!(effects, vec![Effect::Inject("late".into())]);
    }

    #[test]
    fn engine_failure_returns_to_idle_and_ignores_stale_events() {
        let mut session = Session::new();
        let id = start(&mut session);
        let effects = session.handle(SessionEvent::EngineFailed {
            id,
            message: "no api key".into(),
        });
        assert_eq!(effects, vec![Effect::EndSession { id }]);
        assert_eq!(session.phase, Phase::Idle);
        assert_eq!(session.last_error.as_deref(), Some("no api key"));

        assert!(session
            .handle(SessionEvent::Transcript {
                id,
                newly_final: "nope".into(),
            })
            .is_empty());
        assert!(session
            .handle(SessionEvent::Finished {
                id,
                remainder: "nope".into(),
            })
            .is_empty());
    }

    #[test]
    fn inject_failure_ends_the_session() {
        let mut session = Session::new();
        let id = start(&mut session);
        session.handle(SessionEvent::EngineReady { id });
        let effects = session.handle(SessionEvent::InjectFailed {
            message: "sendinput failed".into(),
        });
        assert_eq!(effects, vec![Effect::EndSession { id }]);
        assert_eq!(session.phase, Phase::Idle);
        assert_eq!(session.last_error.as_deref(), Some("sendinput failed"));
    }

    #[test]
    fn events_from_an_older_session_are_ignored() {
        let mut session = Session::new();
        let first = start(&mut session);
        session.handle(SessionEvent::EngineFailed {
            id: first,
            message: "boom".into(),
        });
        let second = start(&mut session);
        assert_ne!(first, second);
        session.handle(SessionEvent::EngineReady { id: second });
        assert!(session
            .handle(SessionEvent::Transcript {
                id: first,
                newly_final: "stale".into(),
            })
            .is_empty());
        assert_eq!(session.phase, Phase::Listening { id: second });
    }

    #[test]
    fn finished_while_still_listening_completes() {
        let mut session = Session::new();
        let id = start(&mut session);
        session.handle(SessionEvent::EngineReady { id });
        let effects = session.handle(SessionEvent::Finished {
            id,
            remainder: "done".into(),
        });
        assert_eq!(effects, vec![Effect::Inject("done".into())]);
        assert_eq!(session.phase, Phase::Idle);
    }

    #[test]
    fn stop_during_finalize_does_not_restart_end() {
        let mut session = Session::new();
        let id = start(&mut session);
        session.handle(SessionEvent::Stop);
        assert!(session.handle(SessionEvent::Stop).is_empty());
        assert_eq!(session.phase, Phase::Finalizing { id });
    }
}

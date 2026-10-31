//! Key handling for the performance overview.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use vim_rs::types::structs::ManagedObjectReference;

use crate::body_pane::{BodyKeyResult, PaneAction};
use crate::perf_overview::layout::Domain;
use crate::perf_overview::render;
use crate::perf_overview::store::PerfOverviewShare;

pub struct PerfOverview {
    pub moref: ManagedObjectReference,
    pub focus: Domain,
    pub store: PerfOverviewShare,
}

impl PerfOverview {
    pub fn new(moref: ManagedObjectReference, store: PerfOverviewShare) -> Self {
        Self {
            moref,
            focus: Domain::Cpu,
            store,
        }
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> BodyKeyResult {
        let outcome = key_outcome(key.code, self.focus);
        if let Some(focus) = outcome.focus {
            self.focus = focus;
        }
        outcome.result
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        render::draw(frame, area, self);
    }
}

struct KeyOutcome {
    result: BodyKeyResult,
    focus: Option<Domain>,
}

fn key_outcome(code: KeyCode, focus: Domain) -> KeyOutcome {
    match code {
        KeyCode::Esc => KeyOutcome {
            result: BodyKeyResult {
                handled: true,
                new_perf_view: false,
                action: Some(PaneAction::Back),
            },
            focus: None,
        },
        KeyCode::Backspace | KeyCode::Char('q') => KeyOutcome {
            result: BodyKeyResult {
                handled: false,
                new_perf_view: false,
                action: None,
            },
            focus: None,
        },
        KeyCode::Char('r') | KeyCode::Char('s') | KeyCode::Char('/') => KeyOutcome {
            result: BodyKeyResult {
                handled: true,
                new_perf_view: false,
                action: None,
            },
            focus: None,
        },
        KeyCode::Tab => focused(cycle(focus, true)),
        KeyCode::BackTab => focused(cycle(focus, false)),
        KeyCode::Char('1') => focused(Domain::Cpu),
        KeyCode::Char('2') => focused(Domain::Memory),
        KeyCode::Char('3') => focused(Domain::Network),
        KeyCode::Char('4') => focused(Domain::Disk),
        _ => KeyOutcome {
            result: BodyKeyResult {
                handled: false,
                new_perf_view: false,
                action: None,
            },
            focus: None,
        },
    }
}

fn focused(focus: Domain) -> KeyOutcome {
    KeyOutcome {
        result: BodyKeyResult {
            handled: true,
            new_perf_view: false,
            action: None,
        },
        focus: Some(focus),
    }
}

fn cycle(focus: Domain, forward: bool) -> Domain {
    let index = Domain::ALL
        .iter()
        .position(|domain| *domain == focus)
        .unwrap_or(0);
    let next = if forward {
        (index + 1) % Domain::ALL.len()
    } else {
        (index + Domain::ALL.len() - 1) % Domain::ALL.len()
    };
    Domain::ALL[next]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn esc_goes_back_and_quit_keys_are_not_consumed() {
        let esc = key_outcome(KeyCode::Esc, Domain::Cpu);
        assert_eq!(esc.result.action, Some(PaneAction::Back));
        assert!(esc.result.handled);

        for code in [KeyCode::Backspace, KeyCode::Char('q')] {
            let outcome = key_outcome(code, Domain::Cpu);
            assert!(!outcome.result.handled, "{code:?}");
            assert!(outcome.result.action.is_none());
        }
    }

    #[test]
    fn inventory_keys_are_consumed_without_an_action() {
        for code in [KeyCode::Char('r'), KeyCode::Char('s'), KeyCode::Char('/')] {
            let outcome = key_outcome(code, Domain::Network);
            assert!(outcome.result.handled);
            assert!(outcome.result.action.is_none());
            assert!(outcome.focus.is_none());
        }
    }

    #[test]
    fn tab_and_digits_move_focus() {
        for code in [
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Char('1'),
            KeyCode::Char('2'),
            KeyCode::Char('3'),
            KeyCode::Char('4'),
        ] {
            let outcome = key_outcome(code, Domain::Cpu);
            assert!(outcome.result.handled, "{code:?}");
            assert!(outcome.focus.is_some(), "{code:?}");
        }
        assert_eq!(
            key_outcome(KeyCode::Char('3'), Domain::Cpu).focus,
            Some(Domain::Network)
        );
        assert_eq!(cycle(Domain::Disk, true), Domain::Cpu);
        assert_eq!(cycle(Domain::Cpu, false), Domain::Disk);
        let _ = press(KeyCode::Tab);
    }
}

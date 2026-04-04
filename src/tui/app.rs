//! TUI event loop.
//!
//! Sets up a raw-mode alternate-screen terminal via [`ratatui::init`], enters a
//! poll-based loop that refreshes the clock, renders via [`render_to_buffer`],
//! and dispatches keyboard events to [`AppState`] methods. Modal dialogs and the
//! help overlay intercept keys before the main keybinding table.

use anyhow::Result;
use chrono::Utc;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

use crate::core::model::ComparisonModel;
use crate::tui::state::AppState;
use crate::tui::view::render_to_buffer;

/// Enter the interactive TUI, blocking until the user quits.
pub fn run(model: ComparisonModel) -> Result<()> {
    let mut state = initial_state(model);
    let mut terminal = ratatui::init();
    let _guard = TerminalGuard;

    loop {
        state.refresh_now(Utc::now())?;

        terminal.draw(|frame| {
            let area = frame.area();
            render_to_buffer(frame.buffer_mut(), area, &state);
        })?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        if state.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => state.show_help = false,
                _ => {}
            }
            continue;
        }

        if let Some(modal) = &state.modal {
            let is_edit_window = matches!(modal, crate::tui::forms::Modal::EditWindow { .. });
            match key.code {
                KeyCode::Esc => state.cancel_modal(),
                KeyCode::Enter => {
                    if let Err(error) = state.submit_modal() {
                        state.status = Some(error.to_string());
                    }
                }
                KeyCode::Tab if is_edit_window => state.edit_window_switch_pane(),
                KeyCode::Up | KeyCode::Char('k') if is_edit_window => state.edit_window_up(),
                KeyCode::Down | KeyCode::Char('j') if is_edit_window => state.edit_window_down(),
                KeyCode::Backspace if !is_edit_window => state.pop_modal_char(),
                KeyCode::Up if !is_edit_window => state.picker_up(),
                KeyCode::Down if !is_edit_window => state.picker_down(),
                KeyCode::Char(ch) if !is_edit_window => state.push_modal_char(ch),
                _ => {}
            }
            continue;
        }

        match key.code {
            KeyCode::Char('q') => break,
            KeyCode::Char('o') => state.cycle_sort_mode(),
            KeyCode::Char('a') => state.open_add_zone(),
            KeyCode::Char('x') => {
                if state.selected_zone == 0 {
                    // UTC row — no-op
                } else if let Err(error) = state.remove_zone(state.selected_zone) {
                    state.status = Some(error.to_string());
                }
            }
            KeyCode::Char('J') => state.move_zone_down(state.selected_zone),
            KeyCode::Char('K') => state.move_zone_up(state.selected_zone),
            KeyCode::Char('e') if state.selected_zone != 0 => {
                state.open_edit_window();
            }
            KeyCode::Char('s') => {
                state.status = Some(match state.save() {
                    Ok(()) => "Saved config".to_string(),
                    Err(error) => error.to_string(),
                });
            }
            KeyCode::Left | KeyCode::Char('h') => state.focus_left(),
            KeyCode::Right | KeyCode::Char('l') => state.focus_right(),
            KeyCode::Up | KeyCode::Char('k') => state.focus_up(),
            KeyCode::Down | KeyCode::Char('j') => state.focus_down(),
            KeyCode::Char('?') => state.show_help = true,
            _ => {}
        }
    }

    Ok(())
}

/// Build the initial application state using the current wall-clock time.
fn initial_state(model: ComparisonModel) -> AppState {
    AppState::new(model, Utc::now())
}

/// RAII guard that restores terminal state on drop via [`ratatui::restore`].
///
/// Ensures the terminal is cleaned up after a panic or early return.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration as ChronoDuration, Utc};

    use crate::core::model::ComparisonModel;

    #[test]
    fn initial_state_uses_wall_clock_now_instead_of_anchor_time() {
        let before = Utc::now();
        let anchor = before + ChronoDuration::hours(4);
        let model = ComparisonModel::from_session(
            crate::core::model::SessionConfig {
                base_zones: vec!["UTC".to_string()],
                extra_zones: Vec::new(),
                ordered_zones: vec!["UTC".to_string()],
                nhours: 12,
                anchor: crate::core::model::AnchorSpec::Explicit(anchor.time()),
                width: Some(96),
                plain: false,
                save_path: std::env::temp_dir().join("ztl-test-app-state.toml"),
                default_window: "09:00-17:00".to_string(),
                work_hours: Default::default(),
                shoulder_hours: 1,
                sort_mode: crate::core::model::SortMode::default(),
            },
            before,
        )
        .unwrap();

        let state = initial_state(model);
        let after = Utc::now();
        assert!(state.now_utc >= before);
        assert!(state.now_utc <= after);
        assert_ne!(state.now_utc, state.model.anchor);
    }
}

//! VM power action popup, confirmation dialog, and rendering.

use crate::operation_types::OperationId;
use crate::vm_power_actions::{VmActionContext, VmPowerAction};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};

/// Modal stack for VM actions (mutually exclusive with error popup handling in `App`).
#[derive(Debug, Default)]
pub struct VmActionUi {
    layer: VmActionLayer,
}

#[derive(Debug, Default)]
enum VmActionLayer {
    #[default]
    Closed,
    /// Prefetch running in the ops worker.
    LoadingPrefetch { request_id: OperationId },
    Menu {
        ctx: VmActionContext,
        actions: Vec<VmPowerAction>,
        selected: usize,
    },
    Confirm {
        ctx: VmActionContext,
        action: VmPowerAction,
    },
}

#[derive(Debug)]
pub enum VmActionKeyOutcome {
    /// Key was for this UI (consumed).
    Consumed,
    /// Let other handlers run.
    Ignored,
    /// Run this action against `vm` (async in `App`).
    Execute {
        vm: vim_rs::types::structs::ManagedObjectReference,
        action: VmPowerAction,
    },
    /// Close all VM action modals.
    Close,
}

impl VmActionUi {
    pub fn is_active(&self) -> bool {
        !matches!(self.layer, VmActionLayer::Closed)
    }

    pub fn open_menu(&mut self, ctx: VmActionContext, actions: Vec<VmPowerAction>) {
        self.layer = VmActionLayer::Menu {
            ctx,
            actions,
            selected: 0,
        };
    }

    pub fn start_prefetch_loading(&mut self, request_id: OperationId) {
        self.layer = VmActionLayer::LoadingPrefetch { request_id };
    }

    /// Returns `true` if this layer is still showing prefetch for `request_id`.
    pub fn prefetch_is_pending(&self, request_id: OperationId) -> bool {
        matches!(
            &self.layer,
            VmActionLayer::LoadingPrefetch { request_id: rid } if *rid == request_id
        )
    }

    pub fn close(&mut self) {
        self.layer = VmActionLayer::Closed;
    }

    pub fn render(&mut self, frame: &mut Frame) {
        match &mut self.layer {
            VmActionLayer::Closed => {}
            VmActionLayer::LoadingPrefetch { .. } => {
                let popup_area = centered_rect(52, 7, frame.area());
                let paragraph = Paragraph::new("\n  Loading VM actions…")
                    .block(
                        Block::default()
                            .title("VM actions")
                            .style(Style::default().bg(Color::DarkGray))
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::Yellow))
                            .title_bottom(Line::from("Esc close")),
                    )
                    .alignment(Alignment::Center);
                frame.render_widget(Clear, popup_area);
                frame.render_widget(paragraph, popup_area);
            }
            VmActionLayer::Menu {
                ctx,
                actions,
                selected,
            } => {
                let title = format!("VM actions — {}", ctx.inventory_path);
                let height = if actions.is_empty() {
                    6u16
                } else {
                    (actions.len() as u16).saturating_add(4).min(18)
                };
                let popup_area = centered_rect(50, height, frame.area());

                let block = Block::default()
                    .title(title)
                    .style(Style::default().bg(Color::DarkGray))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title_bottom(Line::from("Esc close  Enter run/confirm"));

                if actions.is_empty() {
                    let text = Paragraph::new(format!(
                        "{}\n\nNo power actions available for this VM in the current state.",
                        ctx.inventory_path
                    ))
                    .block(block)
                    .wrap(Wrap { trim: true });
                    frame.render_widget(Clear, popup_area);
                    frame.render_widget(text, popup_area);
                } else {
                    let items: Vec<ListItem> = actions
                        .iter()
                        .map(|a| ListItem::new(a.label().to_string()))
                        .collect();
                    let list = List::new(items)
                        .block(block)
                        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                        .highlight_symbol("> ");
                    let mut state = ListState::default();
                    state.select(Some(*selected));
                    frame.render_widget(Clear, popup_area);
                    frame.render_stateful_widget(list, popup_area, &mut state);
                }
            }
            VmActionLayer::Confirm { ctx, action } => {
                let popup_area = centered_rect(58, 7, frame.area());
                let body = format!(
                    "\nPath: {}\n\nAction: {}",
                    ctx.inventory_path,
                    action.label()
                );
                let paragraph = Paragraph::new(body)
                    .block(
                        Block::default()
                            .title("Confirm action")
                            .style(Style::default().bg(Color::DarkGray))
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(Color::Yellow))
                            .title_bottom(Line::from(vec![
                                Span::styled(
                                    " < Enter confirm >",
                                    Style::default().fg(Color::White),
                                ),
                                Span::raw("    "),
                                Span::styled("< Esc back > ", Style::default().fg(Color::White)),
                            ])),
                    )
                    .wrap(Wrap { trim: true })
                    .alignment(Alignment::Left);
                frame.render_widget(Clear, popup_area);
                frame.render_widget(paragraph, popup_area);
            }
        }
    }

    pub fn handle_key(&mut self, key: &KeyEvent) -> VmActionKeyOutcome {
        match &mut self.layer {
            VmActionLayer::Closed => VmActionKeyOutcome::Ignored,
            VmActionLayer::LoadingPrefetch { .. } => match key.code {
                KeyCode::Esc => {
                    self.close();
                    VmActionKeyOutcome::Close
                }
                _ => VmActionKeyOutcome::Consumed,
            },
            VmActionLayer::Menu {
                ctx,
                actions,
                selected,
            } => match key.code {
                KeyCode::Esc => {
                    self.close();
                    VmActionKeyOutcome::Close
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !actions.is_empty() && *selected > 0 {
                        *selected -= 1;
                    }
                    VmActionKeyOutcome::Consumed
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !actions.is_empty() && *selected + 1 < actions.len() {
                        *selected += 1;
                    }
                    VmActionKeyOutcome::Consumed
                }
                KeyCode::Enter => {
                    if actions.is_empty() {
                        self.close();
                        VmActionKeyOutcome::Close
                    } else {
                        let action = actions[*selected];
                        if action.requires_confirmation() {
                            let ctx = ctx.clone();
                            self.layer = VmActionLayer::Confirm { ctx, action };
                            VmActionKeyOutcome::Consumed
                        } else {
                            let vm = ctx.vm.clone();
                            self.close();
                            VmActionKeyOutcome::Execute { vm, action }
                        }
                    }
                }
                _ => VmActionKeyOutcome::Consumed,
            },
            VmActionLayer::Confirm { ctx, action } => match key.code {
                KeyCode::Esc => {
                    let ctx = ctx.clone();
                    let actions = VmPowerAction::visible(&ctx.disabled_method);
                    let selected = actions.iter().position(|a| *a == *action).unwrap_or(0);
                    self.layer = VmActionLayer::Menu {
                        ctx,
                        actions,
                        selected,
                    };
                    VmActionKeyOutcome::Consumed
                }
                KeyCode::Enter => {
                    let vm = ctx.vm.clone();
                    let action = *action;
                    self.close();
                    VmActionKeyOutcome::Execute { vm, action }
                }
                _ => VmActionKeyOutcome::Consumed,
            },
        }
    }
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_width = (r.width * percent_x / 100).max(40);
    let popup_height = height.min(r.height.saturating_sub(2)).max(5);
    Rect {
        x: r.x + (r.width.saturating_sub(popup_width)) / 2,
        y: r.y + (r.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    }
}

pub fn render_error_popup(frame: &mut Frame, message: &str) {
    let area = centered_rect(70, 14, frame.area());
    let paragraph = Paragraph::new(message.to_string())
        .block(
            Block::default()
                .title("Error")
                .style(Style::default().bg(Color::Red))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title_bottom(Line::from("Esc or Enter dismiss")),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

pub fn error_popup_handle_key(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Esc | KeyCode::Enter)
}

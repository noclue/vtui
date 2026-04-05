use crate::event::EventHandler;
use crate::prop_browser::{PropertyBrowserManager, StaticPropertyBrowserManager};
use crate::resource_browser::ResourceManager;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

// Variant names intentionally end with `Browser` to match the widget role in the UI.
#[allow(clippy::enum_variant_names)]
pub(crate) enum BodyPane {
    ResourceBrowser(Box<ResourceManager>),
    PropertyBrowser(PropertyBrowserManager),
    StaticPropertyBrowser(StaticPropertyBrowserManager),
}

/// Unified key handling result for [`BodyPane::handle_key`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct BodyKeyResult {
    pub handled: bool,
    pub new_perf_view: bool,
}

impl BodyPane {
    pub fn render(&mut self, frame: &mut Frame, body_area: Rect) {
        match self {
            BodyPane::ResourceBrowser(resource_manager) => {
                resource_manager.render(frame, body_area);
            }
            BodyPane::PropertyBrowser(property_browser) => {
                property_browser.render(frame, body_area);
            }
            BodyPane::StaticPropertyBrowser(static_browser) => {
                static_browser.render(frame, body_area);
            }
        }
    }

    pub async fn handle_key(
        &mut self,
        key: &KeyEvent,
        events: &mut EventHandler,
    ) -> anyhow::Result<BodyKeyResult> {
        match self {
            BodyPane::ResourceBrowser(resource_manager) => {
                let r = resource_manager.handle_key(key, events).await?;
                Ok(BodyKeyResult {
                    handled: r.handled,
                    new_perf_view: r.new_perf_view,
                })
            }
            BodyPane::PropertyBrowser(property_browser) => {
                let handled = property_browser.handle_key(key, events).await?;
                Ok(BodyKeyResult {
                    handled,
                    new_perf_view: false,
                })
            }
            BodyPane::StaticPropertyBrowser(static_browser) => {
                let handled = static_browser.handle_key(key, events).await?;
                Ok(BodyKeyResult {
                    handled,
                    new_perf_view: false,
                })
            }
        }
    }
    pub fn get_hints(&self) -> (&'static [&'static str], &'static [&'static str]) {
        match self {
            BodyPane::ResourceBrowser(resource_manager) => resource_manager.get_hints(),
            BodyPane::PropertyBrowser(property_browser) => property_browser.get_hints(),
            BodyPane::StaticPropertyBrowser(static_browser) => static_browser.get_hints(),
        }
    }
}

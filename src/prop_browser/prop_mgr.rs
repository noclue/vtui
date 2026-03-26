use crate::event::{AppEvent, EventHandler};
use crate::prop_browser::browser::{BrowserMetadata, PropertyBrowser, PropertyBrowserState};
use crossterm::event::{KeyCode, KeyEvent};
use log::{debug, warn};
use miniserde::json::Object;
use ratatui::Frame;
use ratatui::layout::Rect;
use std::cell::RefCell;
use std::ops::DerefMut;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use tui_tree_widget::TreeState;
use vim_rs::core::pc_cache::{CacheManager, ReadWriteCacheProxy};
use vim_rs::types::structs::{ManagedObjectReference, ObjectSpec};

pub struct PropertyBrowserManager {
    cache_mgr: Rc<RefCell<CacheManager>>,
    filter: ManagedObjectReference,
    browser_state: Arc<RwLock<PropertyBrowserState>>,
    obj: ManagedObjectReference,
}

#[derive(Debug)]
pub enum PropertyHistoryRecord {
    Managed {
        obj: ManagedObjectReference,
        state: TreeState<String>,
    },
    Static {
        metadata: BrowserMetadata,
        root: Object,
        state: TreeState<String>,
    },
}

pub struct StaticPropertyBrowserManager {
    browser_state: Arc<RwLock<PropertyBrowserState>>,
}

impl StaticPropertyBrowserManager {
    pub fn new(metadata: BrowserMetadata, root: Object) -> anyhow::Result<Self> {
        let browser_state = Arc::new(RwLock::new(PropertyBrowserState::from_static_json(
            metadata,
            root,
            None,
        )?));
        Ok(Self { browser_state })
    }

    pub fn from_history(
        metadata: BrowserMetadata,
        root: Object,
        state: TreeState<String>,
    ) -> anyhow::Result<Self> {
        let browser_state = Arc::new(RwLock::new(PropertyBrowserState::from_static_json(
            metadata,
            root,
            Some(state),
        )?));
        Ok(Self { browser_state })
    }

    pub fn load_history_record(
        &mut self,
        metadata: BrowserMetadata,
        root: Object,
        state: TreeState<String>,
    ) -> anyhow::Result<()> {
        let _ = self
            .browser_state
            .write()
            .expect("PropertyBrowserState lock poisoned")
            .load_json_root(metadata, None, root, Some(state));
        Ok(())
    }

    pub fn save_state(&mut self, events: &mut EventHandler) {
        let mut g = self
            .browser_state
            .write()
            .expect("PropertyBrowserState lock poisoned");
        let tree_state = g.replace_tree_state(None);
        let (metadata, root) = g.static_history_snapshot();
        events.send(AppEvent::PropertyManagerHistory(PropertyHistoryRecord::Static {
            metadata,
            root,
            state: tree_state,
        }));
    }

    pub fn render(&mut self, frame: &mut Frame, body_area: Rect) {
        let props = PropertyBrowser::new();
        let mut state = self
            .browser_state
            .write()
            .expect("PropertyBrowserState lock poisoned");
        frame.render_stateful_widget(props, body_area, state.deref_mut());
    }

    pub async fn handle_key(
        &mut self,
        key: &KeyEvent,
        events: &mut EventHandler,
    ) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Char('w') | KeyCode::Up => {
                self.browser_state
                    .write()
                    .expect("PropertyBrowserState lock poisoned")
                    .up();
            }
            KeyCode::Char('s') | KeyCode::Down => {
                self.browser_state
                    .write()
                    .expect("PropertyBrowserState lock poisoned")
                    .down();
            }
            KeyCode::Char('a') | KeyCode::Left => {
                self.browser_state
                    .write()
                    .expect("PropertyBrowserState lock poisoned")
                    .left();
            }
            KeyCode::Char('d') | KeyCode::Right => {
                self.browser_state
                    .write()
                    .expect("PropertyBrowserState lock poisoned")
                    .right();
            }
            KeyCode::Enter => {
                let selected = self
                    .browser_state
                    .read()
                    .expect("PropertyBrowserState lock poisoned")
                    .get_selected_object();
                let Some(selected) = selected else {
                    return Ok(false);
                };
                self.save_state(events);
                events.send(AppEvent::LoadProperties(selected));
            }
            KeyCode::Char('j') => {
                self.browser_state
                    .read()
                    .expect("PropertyBrowserState lock poisoned")
                    .dump_to_json()?;
            }
            _ => {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn get_hints(&self) -> (&'static [&'static str], &'static [&'static str]) {
        (&[], &["q quit", "r resource", "j dump json", "Enter open"])
    }
}

impl PropertyBrowserManager {
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn new(
        cache_mgr: Rc<RefCell<CacheManager>>,
        obj: ManagedObjectReference,
    ) -> anyhow::Result<Self> {
        let browser_state = Arc::new(RwLock::new(
            PropertyBrowserState::new(obj.clone(), None).await?,
        ));

        let filter = cache_mgr
            .borrow_mut()
            .add_cache(
                Box::new(ReadWriteCacheProxy::new(browser_state.clone())),
                vec![ObjectSpec {
                    obj: obj.clone(),
                    skip: Some(false),
                    select_set: None,
                }],
            )
            .await?;

        Ok(Self {
            cache_mgr,
            filter,
            browser_state,
            obj,
        })
    }

    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn from_history_record(
        record: PropertyHistoryRecord,
        cache_mgr: Rc<RefCell<CacheManager>>,
    ) -> anyhow::Result<Self> {
        let PropertyHistoryRecord::Managed { obj, state } = record else {
            anyhow::bail!("expected managed property history record");
        };
        let browser_state = Arc::new(RwLock::new(
            PropertyBrowserState::new(obj.clone(), Some(state)).await?,
        ));

        let filter = cache_mgr
            .borrow_mut()
            .add_cache(
                Box::new(ReadWriteCacheProxy::new(browser_state.clone())),
                vec![ObjectSpec {
                    obj: obj.clone(),
                    skip: Some(false),
                    select_set: None,
                }],
            )
            .await?;

        let mgr = Self {
            cache_mgr,
            filter,
            browser_state,
            obj,
        };

        Ok(mgr)
    }

    pub async fn load_history_record(
        &mut self,
        entry: PropertyHistoryRecord,
    ) -> anyhow::Result<()> {
        let PropertyHistoryRecord::Managed { obj, state } = entry else {
            anyhow::bail!("expected managed property history record");
        };
        let _ = self.load_int(obj, Some(state)).await?;
        Ok(())
    }

    pub fn save_state(&mut self, events: &mut EventHandler) {
        let tree_state = self
            .browser_state
            .write()
            .expect("PropertyBrowserState lock poisoned")
            .replace_tree_state(None);
        let entry = PropertyHistoryRecord::Managed {
            obj: self.obj.clone(),
            state: tree_state,
        };
        events.send(AppEvent::PropertyManagerHistory(entry));
    }

    pub fn render(&mut self, frame: &mut Frame, body_area: Rect) {
        let props = PropertyBrowser::new();
        let mut state = self
            .browser_state
            .write()
            .expect("PropertyBrowserState lock poisoned");
        frame.render_stateful_widget(props, body_area, state.deref_mut());
    }

    pub async fn handle_key(
        &mut self,
        key: &KeyEvent,
        events: &mut EventHandler,
    ) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Char('w') | KeyCode::Up => {
                self.browser_state
                    .write()
                    .expect("PropertyBrowserState lock poisoned")
                    .up();
            }
            KeyCode::Char('s') | KeyCode::Down => {
                self.browser_state
                    .write()
                    .expect("PropertyBrowserState lock poisoned")
                    .down();
            }
            KeyCode::Char('a') | KeyCode::Left => {
                self.browser_state
                    .write()
                    .expect("PropertyBrowserState lock poisoned")
                    .left();
            }
            KeyCode::Char('d') | KeyCode::Right => {
                self.browser_state
                    .write()
                    .expect("PropertyBrowserState lock poisoned")
                    .right();
            }
            KeyCode::Enter => {
                self.enter(events).await?;
            }
            KeyCode::Char('j') => {
                self.browser_state
                    .read()
                    .expect("PropertyBrowserState lock poisoned")
                    .dump_to_json()?;
            }
            _ => {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub async fn load(
        &mut self,
        obj: ManagedObjectReference,
        events: &mut EventHandler,
    ) -> anyhow::Result<bool> {
        if self.obj.value == obj.value {
            return Ok(false);
        }
        let old_obj = self.obj.clone();
        let res = self.load_int(obj, None).await?;
        self.add_history(old_obj, res, events);
        Ok(true)
    }
    pub fn get_hints(&self) -> (&'static [&'static str], &'static [&'static str]) {
        (&[], &["q quit", "r resource", "j dump json", "Enter open"])
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn load_int(
        &mut self,
        obj: ManagedObjectReference,
        new_tree_state: Option<TreeState<String>>,
    ) -> anyhow::Result<TreeState<String>> {
        self.cache_mgr
            .borrow_mut()
            .remove_cache(&self.filter)
            .await?;
        self.obj = obj;

        let tree_state = self
            .browser_state
            .write()
            .expect("PropertyBrowserState lock poisoned")
            .set_obj(self.obj.clone(), new_tree_state)?;

        let new_filter = self
            .cache_mgr
            .borrow_mut()
            .add_cache(
                Box::new(ReadWriteCacheProxy::new(self.browser_state.clone())),
                vec![ObjectSpec {
                    obj: self.obj.clone(),
                    skip: Some(false),
                    select_set: None,
                }],
            )
            .await?;
        self.filter = new_filter;

        Ok(tree_state)
    }

    async fn enter(&mut self, events: &mut EventHandler) -> anyhow::Result<bool> {
        let selected = self
            .browser_state
            .read()
            .expect("PropertyBrowserState lock poisoned")
            .get_selected_object();
        let Some(selected) = selected else {
            return Ok(false);
        };

        self.load(selected, events).await?;
        Ok(true)
    }

    fn add_history(
        &mut self,
        selected_object: ManagedObjectReference,
        tree_state: TreeState<String>,
        events: &mut EventHandler,
    ) {
        let entry = PropertyHistoryRecord::Managed {
            obj: selected_object,
            state: tree_state,
        };
        events.send(AppEvent::PropertyManagerHistory(entry));
    }
}

impl Drop for PropertyBrowserManager {
    fn drop(&mut self) {
        let cache_mgr = self.cache_mgr.clone();
        let filter = self.filter.clone();
        tokio::task::block_in_place(|| {
            #[allow(clippy::await_holding_refcell_ref)]
            tokio::runtime::Handle::current().block_on(async move {
                debug!("Terminating PropertyBrowserManager. Releasing filter");
                cache_mgr
                    .borrow_mut()
                    .remove_cache(&filter)
                    .await
                    .unwrap_or_else(|e| {
                        warn!(
                            "Failed to remove PropertyBrowserManager filter: {:?}, {}",
                            filter, e
                        );
                    });
            });
        });
    }
}

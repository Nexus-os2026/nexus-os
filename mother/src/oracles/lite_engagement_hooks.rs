//! Per-instance, one-shot schedule point, immediately after a File engagement.
use std::sync::{Arc, Mutex};

type Hook = (&'static str, Box<dyn FnOnce() + Send>);
#[derive(Clone, Default)]
pub(crate) struct SiteHook(Arc<Mutex<Option<Hook>>>);
impl SiteHook {
    pub(crate) fn arm(&self, site: &'static str, hook: impl FnOnce() + Send + 'static) {
        *self.0.lock().unwrap() = Some((site, Box::new(hook)));
    }
    pub(crate) fn fire(&self, site: &'static str) {
        let hook = {
            let mut slot = self.0.lock().unwrap();
            if slot.as_ref().is_some_and(|(at, _)| *at == site) {
                slot.take()
            } else {
                None
            }
        };
        if let Some((_, hook)) = hook {
            hook();
        }
    }
}

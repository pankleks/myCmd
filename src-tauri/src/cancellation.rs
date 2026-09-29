use crate::error::{FsError, Result};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Default)]
struct State {
    active: HashMap<String, Arc<AtomicBool>>,
    early: VecDeque<String>,
}
#[derive(Clone, Default)]
pub struct Registry(Arc<Mutex<State>>);
pub struct Request {
    registry: Registry,
    id: String,
    cancelled: Arc<AtomicBool>,
}
fn validate(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 128 {
        return Err(FsError::new(
            "invalid_request",
            "Request ID must contain 1 to 128 bytes",
        ));
    }
    Ok(())
}
impl Registry {
    pub fn register(&self, id: String) -> Result<Request> {
        validate(&id)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| FsError::new("io_error", "Cancellation state unavailable"))?;
        if state.active.len() >= 64 || state.active.contains_key(&id) {
            return Err(FsError::new(
                "busy",
                "Too many requests or duplicate request ID",
            ));
        }
        let early = state
            .early
            .iter()
            .position(|pending| pending == &id)
            .and_then(|index| state.early.remove(index))
            .is_some();
        let cancelled = Arc::new(AtomicBool::new(early));
        state.active.insert(id.clone(), cancelled.clone());
        Ok(Request {
            registry: self.clone(),
            id,
            cancelled,
        })
    }
    pub fn cancel(&self, ids: &[String]) -> Result<()> {
        if ids.len() > 64 {
            return Err(FsError::new(
                "invalid_request",
                "At most 64 requests may be cancelled together",
            ));
        }
        for id in ids {
            validate(id)?;
        }
        let mut state = self
            .0
            .lock()
            .map_err(|_| FsError::new("io_error", "Cancellation state unavailable"))?;
        for id in ids {
            if let Some(token) = state.active.get(id) {
                token.store(true, Ordering::Relaxed);
            } else if !state.early.contains(id) {
                if state.early.len() == 64 {
                    state.early.pop_front();
                }
                state.early.push_back(id.clone());
            }
        }
        Ok(())
    }
}
impl Request {
    pub fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err(FsError::new("cancelled", "Directory sizing cancelled"))
        } else {
            Ok(())
        }
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        let mut state = self
            .registry
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.active.remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_is_isolated_and_handles_early_requests() {
        let registry = Registry::default();
        registry.cancel(&["early".into()]).unwrap();
        let early = registry.register("early".into()).unwrap();
        assert_eq!(early.check().unwrap_err().code, "cancelled");
        let other = registry.register("other".into()).unwrap();
        assert!(other.check().is_ok());
        registry.cancel(&["other".into()]).unwrap();
        assert!(other.check().is_err());
        drop(other);
        assert!(registry.register("other".into()).unwrap().check().is_ok());
    }
    #[test]
    fn bounds_requests_and_releases_capacity_after_drop() {
        let registry = Registry::default();
        let mut requests = Vec::new();
        for index in 0..64 {
            requests.push(registry.register(format!("id-{index}")).unwrap());
        }
        assert!(registry.register("extra".into()).is_err());
        requests.pop();
        assert!(registry.register("extra".into()).is_ok());
        for index in 0..100 {
            registry.cancel(&[format!("early-{index}")]).unwrap();
        }
        assert_eq!(registry.0.lock().unwrap().early.len(), 64);
        assert!(registry.cancel(&[String::new()]).is_err());
        assert!(registry.cancel(&vec!["id".into(); 65]).is_err());
    }
}

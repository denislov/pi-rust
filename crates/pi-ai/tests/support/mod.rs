use std::ffi::OsString;
use std::sync::{Arc, Mutex, MutexGuard};

use pi_ai::registry::{self, ApiProvider};

#[allow(dead_code)]
static ENV_LOCK: Mutex<()> = Mutex::new(());
static PROVIDER_LOCK: Mutex<()> = Mutex::new(());

#[allow(dead_code)]
pub struct EnvGuard<'a> {
    _lock: MutexGuard<'a, ()>,
    saved: Vec<(&'static str, Option<OsString>)>,
}

#[allow(dead_code)]
impl EnvGuard<'static> {
    pub fn new(names: &[&'static str]) -> Self {
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = names
            .iter()
            .map(|name| (*name, std::env::var_os(name)))
            .collect();
        Self { _lock: lock, saved }
    }
}

#[allow(dead_code)]
impl EnvGuard<'_> {
    pub fn set(&self, name: &str, value: &str) {
        unsafe {
            std::env::set_var(name, value);
        }
    }

    pub fn remove(&self, name: &str) {
        unsafe {
            std::env::remove_var(name);
        }
    }
}

impl Drop for EnvGuard<'_> {
    fn drop(&mut self) {
        for (name, value) in self.saved.iter().rev() {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

pub struct ProviderGuard<'a> {
    _lock: MutexGuard<'a, ()>,
    saved: Vec<(String, Option<Arc<dyn ApiProvider>>)>,
}

#[allow(dead_code)]
impl ProviderGuard<'static> {
    pub fn for_api(api: impl Into<String>) -> Self {
        let api = api.into();
        let lock = PROVIDER_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = vec![(api.clone(), registry::lookup(&api))];
        Self { _lock: lock, saved }
    }

    #[allow(dead_code)]
    pub fn for_apis<const N: usize>(apis: [&str; N]) -> Self {
        let lock = PROVIDER_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = apis
            .iter()
            .map(|api| ((*api).to_owned(), registry::lookup(api)))
            .collect();
        Self { _lock: lock, saved }
    }

    pub fn clear(api: impl Into<String>) -> Self {
        let api = api.into();
        let guard = Self::for_api(api.clone());
        registry::unregister(&api);
        guard
    }

    pub fn register(api: impl Into<String>, provider: Arc<dyn ApiProvider>) -> Self {
        let api = api.into();
        let guard = Self::for_api(api.clone());
        registry::register(&api, provider);
        guard
    }
}

impl Drop for ProviderGuard<'_> {
    fn drop(&mut self) {
        for (api, provider) in self.saved.iter().rev() {
            match provider {
                Some(provider) => registry::register(api, Arc::clone(provider)),
                None => registry::unregister(api),
            }
        }
    }
}

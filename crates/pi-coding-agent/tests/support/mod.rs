use std::ffi::{OsStr, OsString};
use std::sync::{Arc, Mutex, MutexGuard};

use pi_ai::registry::{self, ApiProvider};

static ENV_LOCK: Mutex<()> = Mutex::new(());
static PROVIDER_REGISTRY_LOCK: Mutex<()> = Mutex::new(());

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

    pub fn with_pi_rust_dir<V: AsRef<OsStr>>(value: V) -> Self {
        let guard = Self::new(&["PI_RUST_DIR"]);
        guard.set_pi_rust_dir(value);
        guard
    }
}

#[allow(dead_code)]
impl EnvGuard<'_> {
    pub fn set<V: AsRef<OsStr>>(&self, name: &str, value: V) {
        unsafe {
            std::env::set_var(name, value);
        }
    }

    pub fn remove(&self, name: &str) {
        unsafe {
            std::env::remove_var(name);
        }
    }

    pub fn set_pi_rust_dir<V: AsRef<OsStr>>(&self, value: V) {
        self.set("PI_RUST_DIR", value);
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
    previous: Vec<(String, Option<Arc<dyn ApiProvider>>)>,
}

#[allow(dead_code)]
impl ProviderGuard<'static> {
    pub fn register(api: impl Into<String>, provider: Arc<dyn ApiProvider>) -> Self {
        Self::register_many(vec![(api.into(), provider)])
    }

    pub fn register_many(providers: Vec<(String, Arc<dyn ApiProvider>)>) -> Self {
        let lock = PROVIDER_REGISTRY_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut previous = Vec::with_capacity(providers.len());
        for (api, provider) in providers {
            let prior = registry::lookup(&api);
            registry::register(&api, provider);
            previous.push((api, prior));
        }
        Self {
            _lock: lock,
            previous,
        }
    }
}

impl Drop for ProviderGuard<'_> {
    fn drop(&mut self) {
        for (api, previous) in self.previous.drain(..).rev() {
            match previous {
                Some(provider) => registry::register(&api, provider),
                None => registry::unregister(&api),
            }
        }
    }
}

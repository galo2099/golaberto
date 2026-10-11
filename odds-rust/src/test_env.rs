//! Thread-local environment overrides for tests that exercise experiment flags.
//!
//! Production readers continue to use `std::env`; this module is compiled only
//! for tests so parallel tests never mutate the process environment.

use std::{
    cell::RefCell,
    collections::HashMap,
    env::{self, VarError},
    ffi::OsString,
    marker::PhantomData,
    rc::Rc,
};

thread_local! {
    static OVERRIDES: RefCell<HashMap<String, Option<OsString>>> = RefCell::new(HashMap::new());
}

/// A scoped override. `None` explicitly masks a process environment value.
/// The `Rc` marker keeps the guard on the thread-local state-owning thread.
pub(crate) struct OverrideGuard {
    previous: Vec<(String, Option<Option<OsString>>)>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(crate) fn scoped(overrides: &[(&str, Option<&str>)]) -> OverrideGuard {
    let owned = overrides
        .iter()
        .map(|(key, value)| (*key, value.map(OsString::from)))
        .collect::<Vec<_>>();
    scoped_os(&owned)
}

fn scoped_os(overrides: &[(&str, Option<OsString>)]) -> OverrideGuard {
    let previous = OVERRIDES.with(|values| {
        let mut values = values.borrow_mut();
        overrides
            .iter()
            .map(|(key, value)| {
                let key = (*key).to_owned();
                let previous = values.get(&key).cloned();
                values.insert(key.clone(), value.clone());
                (key, previous)
            })
            .collect()
    });
    OverrideGuard {
        previous,
        _not_send_or_sync: PhantomData,
    }
}

pub(crate) fn var(key: &str) -> Result<String, VarError> {
    OVERRIDES.with(|values| match values.borrow().get(key) {
        Some(Some(value)) => value.clone().into_string().map_err(VarError::NotUnicode),
        Some(None) => Err(VarError::NotPresent),
        None => env::var(key),
    })
}

impl Drop for OverrideGuard {
    fn drop(&mut self) {
        OVERRIDES.with(|values| {
            let mut values = values.borrow_mut();
            for (key, previous) in self.previous.drain(..).rev() {
                match previous {
                    Some(value) => {
                        values.insert(key, value);
                    }
                    None => {
                        values.remove(&key);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{scoped, scoped_os, var};
    use std::{sync::{Arc, Barrier}, thread};

    const KEY: &str = "CODEX_TEST_ONLY_THREAD_LOCAL_ENV_OVERRIDE_7B93";

    #[test]
    fn nested_overrides_restore_values_and_can_mask_process_environment() {
        let _baseline = scoped(&[(KEY, None)]);
        assert!(matches!(var(KEY), Err(std::env::VarError::NotPresent)));
        let outer = scoped(&[(KEY, Some("outer"))]);
        assert_eq!(var(KEY).unwrap(), "outer");
        {
            let _inner = scoped(&[(KEY, None)]);
            assert!(matches!(var(KEY), Err(std::env::VarError::NotPresent)));
        }
        assert_eq!(var(KEY).unwrap(), "outer");
        drop(outer);
        assert!(matches!(var(KEY), Err(std::env::VarError::NotPresent)));
    }

    #[test]
    fn panic_unwind_restores_the_outer_override() {
        let _baseline = scoped(&[(KEY, None)]);
        let outer = scoped(&[(KEY, Some("before-panic"))]);
        let panic = std::panic::catch_unwind(|| {
            let _inner = scoped(&[(KEY, Some("during-panic"))]);
            assert_eq!(var(KEY).unwrap(), "during-panic");
            panic!("exercise scoped override drop during unwind");
        });
        assert!(panic.is_err());
        assert_eq!(var(KEY).unwrap(), "before-panic");
        drop(outer);
        assert!(matches!(var(KEY), Err(std::env::VarError::NotPresent)));
    }

    #[test]
    fn overrides_are_isolated_between_parallel_threads() {
        let _baseline = scoped(&[(KEY, None)]);
        let barrier = Arc::new(Barrier::new(2));
        let threads = ["thread-a", "thread-b"].map(|value| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let _guard = scoped(&[(KEY, Some(value))]);
                barrier.wait();
                assert_eq!(var(KEY).unwrap(), value);
                barrier.wait();
            })
        });
        for thread in threads {
            thread.join().unwrap();
        }
        assert!(matches!(var(KEY), Err(std::env::VarError::NotPresent)));
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_override_preserves_std_var_error_semantics() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let _baseline = scoped(&[(KEY, None)]);
        let _invalid = scoped_os(&[(KEY, Some(OsString::from_vec(vec![0xff])))]);
        assert!(matches!(var(KEY), Err(std::env::VarError::NotUnicode(_))));
    }
}

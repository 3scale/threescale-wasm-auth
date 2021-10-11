use super::*;

pub use imp::fetcher_with;

pub struct Fetcher;

impl Fetcher {
    pub fn get() -> Result<Self, Error> {
        imp::initialize().and(Ok(Self))
    }
}

mod imp {
    use std::cell::RefCell;
    use std::sync::Once;

    use super::*;

    thread_local! {
        static FETCHER: RefCell<Option<Vec<ConfigFetcher>>> = RefCell::new(None);
        static FETCHER_INIT: Once = Once::new();
    }

    pub(super) fn initialize() -> Result<(), Error> {
        FETCHER_INIT.with(|once| {
            let mut res = Ok(());
            once.call_once(|| {
                res = FETCHER.with(|fetcher| {
                    let new_fetcher: Vec<ConfigFetcher> = vec![];
                    let _ = fetcher.borrow_mut().replace(new_fetcher);
                    Ok(())
                });
            });
            res
        })
    }

    pub fn fetcher_with<R, F: FnOnce(&mut Vec<ConfigFetcher>) -> R>(f: F) -> R {
        FETCHER.with(|rng| f(rng.borrow_mut().as_mut().unwrap()))
    }
}

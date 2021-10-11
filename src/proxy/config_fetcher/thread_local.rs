use super::*;

// Thread RNG callable from anywhere within the thread.
// Safe to call this from different contexts as well and
// obtain ThreadRng instances, since they will all use
// the same thread local pseudo RNG.
#[inline]
#[allow(dead_code)]
pub fn fetcher_init_fallible() -> Result<Fetcher, Error> {
    Fetcher::get()
}

// Thread RNG callable from anywhere within the thread.
// Safe to call this from different contexts as well and
// obtain ThreadRng instances, since they will all use
// the same thread local pseudo RNG.
//
// Panic: will panic if the thread local RNG could not be initialized.
#[inline]
#[allow(dead_code)]
pub fn fetcher_init() -> Fetcher {
    fetcher_init_fallible().expect("could not initialize thread local configuration fetcher")
}

pub struct Fetcher;

impl Fetcher {
    pub fn get() -> Result<Self, Error> {
        imp::initialize().and(Ok(Self))
    }

    pub fn with<R, F: FnOnce(&mut Vec<ConfigFetcher>) -> R>(f: F) -> R {
        imp::fetcher_with(f)
    }

    pub fn clear() {
        imp::fetcher_with(|vcf| vcf.clear());
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

    pub(super) fn fetcher_with<R, F: FnOnce(&mut Vec<ConfigFetcher>) -> R>(f: F) -> R {
        FETCHER.with(|vcf| f(vcf.borrow_mut().as_mut().unwrap()))
    }
}

use core::convert::TryFrom;

use serde_json::Error;

use super::{ErrorLocation, ErrorLocationError};

impl<'e> TryFrom<&'e Error> for ErrorLocation<'e, Error> {
    type Error = ErrorLocationError;

    fn try_from(err: &'e serde_json::Error) -> Result<Self, Self::Error> {
        ErrorLocation::new(
            err,
            err.line(),
            err.column(),
            Some(format!("{:?}", err.classify()).as_str()),
        )
    }
}

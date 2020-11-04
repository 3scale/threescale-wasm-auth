use core::convert::TryFrom;

use super::{ErrorLocation, ErrorLocationError};
use serde_yaml::Error;

impl<'e> TryFrom<&'e Error> for ErrorLocation<'e, Error> {
    type Error = ErrorLocationError;

    fn try_from(err: &'e serde_yaml::Error) -> Result<Self, Self::Error> {
        let location = err
            .location()
            .ok_or(ErrorLocationError::NoLocationAvailable)?;

        // there's not much of a "kind" for this error, so just leave it empty
        ErrorLocation::new(err, location.line(), location.column(), None)
    }
}

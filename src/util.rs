use std::fmt::{Debug, Display};

use apply::Apply;

pub fn err<T, M>(msg: M) -> anyhow::Result<T> where
    M: Display + Debug + Send + Sync + 'static {
    anyhow::Error::msg(msg)
        .apply(Err)
}

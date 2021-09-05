use proc_macro2::TokenStream as TokenStream2;
use std::fmt::{Display, Formatter, Result as FmtResult};
use syn::Error as SynError;

#[derive(Debug, Clone)]
pub enum Error {
    SynError(SynError),
    CompoundError(Vec<Error>),
}

impl Error {
    pub fn into_compile_errors(self) -> Vec<TokenStream2> {
        match self {
            Self::SynError(x) => vec![x.into_compile_error()],
            Self::CompoundError(x) => x
                .into_iter()
                .flat_map(|e| match e {
                    Self::SynError(s) => vec![s.into_compile_error()],
                    Self::CompoundError(_) => e.into_compile_errors(),
                })
                .collect(),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SynError(x) => x.source(),
            Self::CompoundError(x) => x.first().and_then(std::error::Error::source),
        }
    }
}

impl From<SynError> for Error {
    fn from(syn_error: SynError) -> Error {
        Error::SynError(syn_error)
    }
}

impl From<Vec<Error>> for Error {
    fn from(errors: Vec<Error>) -> Error {
        Error::CompoundError(errors)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{:?}", self)
    }
}

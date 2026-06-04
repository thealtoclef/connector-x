use thiserror::Error;
use url;

#[derive(Error, Debug)]
pub enum SpannerSourceError {
    #[error(transparent)]
    ConnectorXError(#[from] crate::errors::ConnectorXError),

    #[error(transparent)]
    SpannerError(#[from] google_cloud_spanner::Error),

    #[error(transparent)]
    SpannerUrlError(#[from] url::ParseError),

    /// Any other errors that are too trivial to be put here explicitly.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

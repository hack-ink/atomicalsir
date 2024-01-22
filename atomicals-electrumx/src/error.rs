//! atomicals-electrumx error collections.

#![allow(missing_docs)]

// crates.io
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
	#[error("exceeded maximum retries")]
	ExceededMaximumRetries,

	#[error(transparent)]
	Bitcoin(#[from] bitcoin::address::Error),
	#[error(transparent)]
	Reqwest(#[from] reqwest::Error),
}

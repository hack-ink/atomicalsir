//! Atomicals electrumx utilities.

// crates.io
use bitcoin::Address;
use sha2::{Digest, Sha256};
// atomicals-electrumx
use crate::prelude::*;

/// Convert an address to a scripthash.
pub fn address2scripthash(address: &Address) -> Result<String> {
	let mut hasher = Sha256::new();

	hasher.update(address.script_pubkey());

	let mut hash = hasher.finalize();

	hash.reverse();

	Ok(array_bytes::bytes2hex("", hash))
}
#[test]
fn address2scripthash_should_work() {
	// std
	use std::str::FromStr;
	// crates.io
	use bitcoin::Network;

	assert_eq!(
		address2scripthash(
			&Address::from_str("bc1pqkq0rg5yjrx6u08nhmc652s33g96jmdz4gjp9d46ew6ahun7xuvqaerzsp")
				.unwrap()
				.require_network(Network::Bitcoin)
				.unwrap()
		)
		.unwrap(),
		"2ae9d6353b5f9b05073e3a4def3b47ab05033d8340ffa6959917c21779f956cf"
	)
}

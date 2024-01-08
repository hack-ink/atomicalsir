// std
use std::{
	collections::HashMap,
	fs::{self, File},
	path::{Path, PathBuf},
};
// crates.io
use serde::Deserialize;
// atomicalsir
use crate::prelude::*;

#[derive(Debug)]
pub struct Wallet {
	pub path: PathBuf,
	pub stash: KeyAlias,
	pub funding: Key,
}
impl Wallet {
	pub fn load<P>(path: P) -> Result<Self>
	where
		P: AsRef<Path>,
	{
		let p = path.as_ref();
		let w = serde_json::from_reader::<_, WalletJson>(File::open(p).unwrap())?;

		Ok(Self {
			path: p.to_path_buf(),
			stash: w
				.imported
				.get("stash")
				.map(|k| KeyAlias { alias: "stash".into(), key: k.to_owned() })
				.unwrap_or(KeyAlias { alias: "primary".into(), key: w.primary.clone() }),
			funding: w.funding,
		})
	}

	pub fn load_wallets<P>(path: P) -> Vec<Wallet>
	where
		P: AsRef<Path>,
	{
		fs::read_dir(path)
			.ok()
			.map(|rd| {
				rd.filter_map(|r| {
					r.ok().and_then(|d| {
						let p = d.path();

						if p.extension().map(|e| e == "json") == Some(true) {
							Wallet::load(&p)
								.map(|w| {
									tracing::info!("loaded wallet: {}", p.display());

									w
								})
								.map_err(|e| {
									tracing::error!(
										"failed to load wallet from {} due to {e}",
										p.display(),
									);

									e
								})
								.ok()
						} else {
							None
						}
					})
				})
				.collect()
			})
			.unwrap_or_default()
	}
}

#[derive(Debug)]
pub struct KeyAlias {
	pub alias: String,
	pub key: Key,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Key {
	pub address: String,
	#[serde(rename = "WIF")]
	pub wif: String,
}

#[derive(Debug, Deserialize)]
struct WalletJson {
	primary: Key,
	funding: Key,
	imported: HashMap<String, Key>,
}

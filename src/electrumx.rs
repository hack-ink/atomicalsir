#[cfg(test)] mod test;

pub mod r#type;
use r#type::*;

// std
use std::{str::FromStr, time::Duration};
// crates.io
use bitcoin::{Address, Network};
use reqwest::{Client as ReqwestClient, ClientBuilder as ReqwestClientBuilder};
use serde::{de::DeserializeOwned, Serialize};
use tokio::time;
// atomicalsir
use crate::{prelude::*, util};

pub trait Config {
	fn network(&self) -> &Network;
	fn base_uri(&self) -> &str;
}

pub trait Http {
	async fn post<U, P, R>(&self, uri: U, params: P) -> Result<R>
	where
		U: AsRef<str>,
		P: Serialize,
		R: DeserializeOwned;
}

pub trait Api: Config + Http {
	fn uri_of<S>(&self, uri: S) -> String
	where
		S: AsRef<str>,
	{
		format!("{}/{}", self.base_uri(), uri.as_ref())
	}

	async fn get_by_ticker<S>(&self, ticker: S) -> Result<Ticker>
	where
		S: AsRef<str>,
	{
		Ok(self
			.post::<_, _, Response<ResponseResult<Ticker>>>(
				self.uri_of("blockchain.atomicals.get_by_ticker"),
				Params::new([ticker.as_ref()]),
			)
			.await?
			.response
			.result)
	}

	async fn get_ft_info<S>(&self, atomical_id: S) -> Result<ResponseResult<Ft>>
	where
		S: AsRef<str>,
	{
		Ok(self
			.post::<_, _, Response<ResponseResult<Ft>>>(
				self.uri_of("blockchain.atomicals.get_ft_info"),
				Params::new([atomical_id.as_ref()]),
			)
			.await?
			.response)
	}

	async fn get_unspent_address<S>(&self, address: S) -> Result<Vec<Utxo>>
	where
		S: AsRef<str>,
	{
		self.get_unspent_scripthash(util::address2scripthash(
			&Address::from_str(address.as_ref()).unwrap().require_network(*self.network())?,
		)?)
		.await
	}

	async fn get_unspent_scripthash<S>(&self, scripthash: S) -> Result<Vec<Utxo>>
	where
		S: AsRef<str>,
	{
		let mut utxos = self
			.post::<_, _, Response<Vec<Unspent>>>(
				self.uri_of("blockchain.scripthash.listunspent"),
				Params::new([scripthash.as_ref()]),
			)
			.await?
			.response
			.into_iter()
			.map(|u| u.into())
			.collect::<Vec<Utxo>>();

		utxos.sort_by(|a, b| a.value.cmp(&b.value));

		Ok(utxos)
	}

	async fn wait_until_utxo<S>(&self, address: S, satoshis: u64) -> Result<Utxo>
	where
		S: AsRef<str>,
	{
		loop {
			for u in self.get_unspent_address(address.as_ref()).await? {
				if u.atomicals.is_empty() && u.value >= satoshis {
					return Ok(u);
				}
			}

			tracing::info!("waiting for UTXO...");

			time::sleep(Duration::from_secs(5)).await;
		}
	}

	// TODO: Return type.
	async fn broadcast<S>(&self, tx: S) -> Result<serde_json::Value>
	where
		S: AsRef<str>,
	{
		self.post::<_, _, serde_json::Value>(
			self.uri_of("blockchain.transaction.broadcast"),
			Params::new([tx.as_ref()]),
		)
		.await
	}
}
impl<T> Api for T where T: Config + Http {}

#[derive(Debug)]
pub struct ElectrumX {
	pub client: ReqwestClient,
	pub network: Network,
	pub base_uri: String,
}
impl Config for ElectrumX {
	fn network(&self) -> &Network {
		&self.network
	}

	fn base_uri(&self) -> &str {
		&self.base_uri
	}
}
impl Http for ElectrumX {
	async fn post<U, P, R>(&self, uri: U, params: P) -> Result<R>
	where
		U: AsRef<str>,
		P: Serialize,
		R: DeserializeOwned,
	{
		let resp = self.client.post(uri.as_ref()).json(&params).send().await?.text().await?;

		tracing::debug!("{resp}");

		Ok(serde_json::from_str(&resp)?)
	}
}

#[derive(Debug)]
pub struct ElectrumXBuilder {
	pub network: Network,
	pub base_uri: String,
}
impl ElectrumXBuilder {
	pub fn network(mut self, network: Network) -> Self {
		self.network = network;

		self
	}

	pub fn base_uri<S>(mut self, base_uri: S) -> Self
	where
		S: Into<String>,
	{
		self.base_uri = base_uri.into();

		self
	}

	pub fn build(self) -> Result<ElectrumX> {
		Ok(ElectrumX {
			client: ReqwestClientBuilder::new().timeout(Duration::from_secs(30)).build()?,
			network: self.network,
			base_uri: self.base_uri,
		})
	}
}
impl Default for ElectrumXBuilder {
	fn default() -> Self {
		Self { network: Network::Bitcoin, base_uri: "https://ep.atomicals.xyz/proxy".into() }
	}
}

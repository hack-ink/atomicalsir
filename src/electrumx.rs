// TODO: Make this a single library.
// TODO: Use thiserror.

#[cfg(test)] mod test;

pub mod r#type;
use r#type::*;

// std
use std::{str::FromStr, time::Duration};
// crates.io
use bitcoin::{Address, Amount, Network};
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
		let addr = address.as_ref();
		let sat = Amount::from_sat(satoshis);

		loop {
			if let Some(u) = self
				.get_unspent_address(addr)
				.await?
				.into_iter()
				.find(|u| u.atomicals.is_empty() && u.value >= satoshis)
			{
				tracing::info!(
					"funding UTXO detected {}:{} with a value of {} for funding purposes",
					u.txid,
					u.vout,
					u.value
				);
				return Ok(u);
			}

			tracing::info!(
				"awaiting UTXO confirmation until {sat} BTC is received at address {addr}"
			);

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
	pub retry_period: Duration,
	pub max_retries: MaxRetries,
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
		let u = uri.as_ref();

		for _ in self.max_retries.clone() {
			match self.client.post(u).json(&params).send().await {
				Ok(r) => match r.json().await {
					Ok(r) => return Ok(r),
					Err(e) => {
						tracing::error!("failed to parse response into JSON due to {e}");
					},
				},
				Err(e) => {
					tracing::error!("the request to {u} failed due to {e}");
				},
			}

			time::sleep(self.retry_period).await;
		}

		Err(anyhow::anyhow!("exceeded maximum retries"))
	}
}

#[derive(Debug)]
pub struct ElectrumXBuilder {
	pub timeout: Duration,
	pub retry_period: Duration,
	pub max_retries: MaxRetries,
	pub network: Network,
	pub base_uri: String,
}
// TODO: Remove this cfg.
#[allow(unused)]
impl ElectrumXBuilder {
	#[cfg(test)]
	pub fn testnet() -> Self {
		Self::default().network(Network::Testnet).base_uri("https://eptestnet.atomicals.xyz/proxy")
	}

	pub fn timeout(mut self, timeout: Duration) -> Self {
		self.timeout = timeout;

		self
	}

	pub fn retry_period(mut self, retry_period: Duration) -> Self {
		self.retry_period = retry_period;

		self
	}

	pub fn max_retries(mut self, max_retries: MaxRetries) -> Self {
		self.max_retries = max_retries;

		self
	}

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
			client: ReqwestClientBuilder::new().timeout(self.timeout).build()?,
			retry_period: self.retry_period,
			max_retries: self.max_retries,
			network: self.network,
			base_uri: self.base_uri,
		})
	}
}
impl Default for ElectrumXBuilder {
	fn default() -> Self {
		Self {
			timeout: Duration::from_secs(30),
			retry_period: Duration::from_secs(5),
			max_retries: MaxRetries::Finite(5),
			network: Network::Bitcoin,
			base_uri: "https://ep.atomicals.xyz/proxy".into(),
		}
	}
}
// TODO: Remove this cfg.
#[allow(unused)]
#[derive(Debug, Clone)]
pub enum MaxRetries {
	Infinite,
	Finite(u8),
}
impl Iterator for MaxRetries {
	type Item = ();

	fn next(&mut self) -> Option<Self::Item> {
		match self {
			Self::Infinite => Some(()),
			Self::Finite(n) =>
				if *n > 0 {
					*n -= 1;

					Some(())
				} else {
					None
				},
		}
	}
}

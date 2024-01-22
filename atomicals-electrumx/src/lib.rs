//! Atomicals electrumx APIs.

#![deny(missing_docs, unused_crate_dependencies)]

#[cfg(test)] mod test;

pub mod error;

pub mod r#type;
use r#type::*;

pub mod util;

pub mod prelude {
	//! atomicals-electrumx prelude.

	pub use std::result::Result as StdResult;

	pub use super::error::{self, Error};

	/// atomicals-electrumx `Result` type.
	pub type Result<T> = StdResult<T, Error>;
}
use prelude::*;

// std
use std::{future::Future, str::FromStr, time::Duration};
// crates.io
use bitcoin::{Address, Amount, Network};
use reqwest::{Client as ReqwestClient, ClientBuilder as ReqwestClientBuilder};
use serde::{de::DeserializeOwned, Serialize};
use tokio::time;

/// Necessary configurations of the client to transform it into an API client.
pub trait Config {
	/// Network type.
	fn network(&self) -> &Network;
	/// Base URI.
	fn base_uri(&self) -> &str;
}

/// Necessary HTTP methods of the client to transform it into an API client.
pub trait Http {
	/// Send a POST request.
	fn post<U, P, R>(&self, uri: U, params: P) -> impl Future<Output = Result<R>> + Send
	where
		U: Send + Sync + AsRef<str>,
		P: Send + Sync + Serialize,
		R: DeserializeOwned;
}

/// Atomicals electrumx APIs.
pub trait Api: Send + Sync + Config + Http {
	/// Construct the API's URI.
	fn uri_of<S>(&self, uri: S) -> String
	where
		S: AsRef<str>,
	{
		format!("{}/{}", self.base_uri(), uri.as_ref())
	}

	/// Make a request at `blockchain.atomicals.get_by_ticker`.
	fn get_by_ticker<S>(&self, ticker: S) -> impl Future<Output = Result<Ticker>> + Send
	where
		S: Send + Sync + AsRef<str>,
	{
		async move {
			Ok(self
				.post::<_, _, Response<ResponseResult<Ticker>>>(
					self.uri_of("blockchain.atomicals.get_by_ticker"),
					[ticker.as_ref()],
				)
				.await?
				.response
				.result)
		}
	}

	/// Make a request at `blockchain.atomicals.get_by_id`.
	fn get_ft_info<S>(
		&self,
		atomical_id: S,
	) -> impl Future<Output = Result<ResponseResult<Ft>>> + Send
	where
		S: Send + Sync + AsRef<str>,
	{
		async move {
			Ok(self
				.post::<_, _, Response<ResponseResult<Ft>>>(
					self.uri_of("blockchain.atomicals.get_ft_info"),
					[atomical_id.as_ref()],
				)
				.await?
				.response)
		}
	}

	/// Make a request at `blockchain.atomicals.get_by_id`.
	fn get_unspent_address<S>(&self, address: S) -> impl Future<Output = Result<Vec<Utxo>>> + Send
	where
		S: Send + Sync + AsRef<str>,
	{
		async move {
			self.get_unspent_scripthash(util::address2scripthash(
				&Address::from_str(address.as_ref()).unwrap().require_network(*self.network())?,
			)?)
			.await
		}
	}

	/// Make a request at `blockchain.scripthash.listunspent`.
	fn get_unspent_scripthash<S>(
		&self,
		scripthash: S,
	) -> impl Future<Output = Result<Vec<Utxo>>> + Send
	where
		S: Send + Sync + AsRef<str>,
	{
		async move {
			let mut utxos = self
				.post::<_, _, Response<Vec<Unspent>>>(
					self.uri_of("blockchain.scripthash.listunspent"),
					[scripthash.as_ref()],
				)
				.await?
				.response
				.into_iter()
				.map(|u| u.into())
				.collect::<Vec<Utxo>>();

			utxos.sort_by(|a, b| a.value.cmp(&b.value));

			Ok(utxos)
		}
	}

	/// Wait until a matching UTXO is found.
	fn wait_until_utxo<S>(
		&self,
		address: S,
		satoshis: u64,
	) -> impl Future<Output = Result<Utxo>> + Send
	where
		S: Send + Sync + AsRef<str>,
	{
		async move {
			let a = address.as_ref();
			let ba = Amount::from_sat(satoshis);

			loop {
				if let Some(u) = self
					.get_unspent_address(a)
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
					"awaiting UTXO confirmation until {ba} BTC is received at address {a}"
				);

				time::sleep(Duration::from_secs(5)).await;
			}
		}
	}

	// TODO: Return type.
	/// Make a request at `blockchain.scripthash.get_balance`.
	fn broadcast<S>(&self, tx: S) -> impl Future<Output = Result<serde_json::Value>> + Send
	where
		S: Send + Sync + AsRef<str>,
	{
		async move {
			self.post::<_, _, serde_json::Value>(
				self.uri_of("blockchain.transaction.broadcast"),
				[tx.as_ref()],
			)
			.await
		}
	}
}
impl<T> Api for T where T: Send + Sync + Config + Http {}

/// Atomicals electrumx client.
#[derive(Debug)]
pub struct ElectrumX {
	/// HTTP client.
	pub client: ReqwestClient,
	/// Retry period.
	pub retry_period: Duration,
	/// Maximum number of retry attempts.
	pub max_retries: MaxRetries,
	/// Network type.
	pub network: Network,
	/// Base URI.
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
		U: Send + Sync + AsRef<str>,
		P: Send + Sync + Serialize,
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

		Err(Error::ExceededMaximumRetries)
	}
}

/// Builder for [`ElectrumX`].
#[derive(Debug)]
pub struct ElectrumXBuilder {
	/// Request timeout.
	pub timeout: Duration,
	/// Retry period.
	pub retry_period: Duration,
	/// Maximum number of retry attempts.
	pub max_retries: MaxRetries,
	/// Network type.
	pub network: Network,
	/// Base URI.
	pub base_uri: String,
}
impl ElectrumXBuilder {
	#[cfg(test)]
	fn testnet() -> Self {
		Self::default().network(Network::Testnet).base_uri("https://eptestnet.atomicals.xyz/proxy")
	}

	/// Set request timeout.
	pub fn timeout(mut self, timeout: Duration) -> Self {
		self.timeout = timeout;

		self
	}

	/// Set retry period.
	pub fn retry_period(mut self, retry_period: Duration) -> Self {
		self.retry_period = retry_period;

		self
	}

	/// Set maximum number of retry attempts.
	pub fn max_retries(mut self, max_retries: MaxRetries) -> Self {
		self.max_retries = max_retries;

		self
	}

	/// Set network type.
	pub fn network(mut self, network: Network) -> Self {
		self.network = network;

		self
	}

	/// Set base URI.
	pub fn base_uri<S>(mut self, base_uri: S) -> Self
	where
		S: Into<String>,
	{
		self.base_uri = base_uri.into();

		self
	}

	/// Build the [`ElectrumX`] client.
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
/// Maximum number of retry attempts.
#[derive(Debug, Clone)]
pub enum MaxRetries {
	/// Unlimited number of retries.
	Infinite,
	/// Limited number of retries.
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

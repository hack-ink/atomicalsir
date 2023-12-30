// std
use std::{ops::Range, path::Path, str::FromStr};
// crates.io
use bitcoin::{
	absolute::LockTime, psbt::Input, transaction::Version, Address, Amount, Network, OutPoint,
	PrivateKey, Psbt, Sequence, Transaction, TxIn, TxOut, Txid, XOnlyPublicKey,
};
use rand::Rng;
use serde::Serialize;
// atomicalsir
use crate::{
	electrumx::{r#type::Utxo, Api, ElectrumX, ElectrumXBuilder},
	prelude::*,
	util,
	wallet::Wallet as RawWallet,
};

pub async fn run(electrumx: &str, wallet_dir: &Path, ticker: &str, max_fee: u64) -> Result<()> {
	let m = MinerBuilder { electrumx, wallet_dir, ticker, max_fee }.build()?;

	// for w in &m.wallets {
	for w in m.wallets.iter().skip(2) {
		m.mine(w).await?;
	}

	Ok(())
}

#[derive(Debug)]
struct Miner {
	api: ElectrumX,
	wallets: Vec<Wallet>,
	ticker: String,
	max_fee: u64,
}
impl Miner {
	const BASE_BYTES: f64 = 10.5;
	const INPUT_BYTES_BASE: f64 = 57.5;
	const OUTPUT_BYTES_BASE: f64 = 43.;
	const REVEAL_INPUT_BYTES_BASE: f64 = 66.;

	async fn mine(&self, wallet: &Wallet) -> Result<()> {
		let d: Data = self.prepare_data(wallet).await?;

		tracing::info!("{d:?}");

		let Data {
			satsbyte,
			op_type,
			bitwork_info_commit,
			dmt_option,
			additional_outputs,
			script_p2tr,
			fees,
			funding_utxo,
		} = d;

		let output = TxOut {
			value: Amount::from_sat(fees.reveal_and_outputs),
			script_pubkey: script_p2tr.script_pubkey(),
		};
		let fee_diff = funding_utxo.value.saturating_sub(fees.reveal_and_outputs).saturating_sub(
			fees.commit + (Self::OUTPUT_BYTES_BASE * satsbyte as f64).floor() as u64,
		);

		#[allow(clippy::never_loop)]
		for s in Self::sequence_chunks() {
			for s in s {
				let mut psbt = Psbt::from_unsigned_tx(Transaction {
					version: Version::ONE,
					lock_time: LockTime::ZERO,
					input: vec![TxIn {
						previous_output: OutPoint::new(
							Txid::from_str(&funding_utxo.txid).unwrap(),
							funding_utxo.vout,
						),
						sequence: Sequence(s),
						..Default::default()
					}],
					output: {
						let mut os = vec![output.clone()];

						if fee_diff > 0 {
							os.push(TxOut {
								value: Amount::from_sat(fee_diff),
								script_pubkey: wallet.funding.address.script_pubkey(),
							})
						}

						os
					},
				})
				.unwrap();

				psbt.inputs.push(Input {
					tap_internal_key: Some(wallet.funding.x_only_public),
					witness_utxo: Some(TxOut {
						value: Amount::from_sat(funding_utxo.value),
						script_pubkey: wallet.funding.address.script_pubkey(),
					}),
					..Default::default()
				});

				dbg!(psbt);
				// psbt.sighash_ecdsa(0, cache);
			}

			break;
		}

		Ok(())
	}

	async fn prepare_data(&self, wallet: &Wallet) -> Result<Data> {
		let id = self.api.get_by_ticker(&self.ticker).await?.atomical_id;
		let response = self.api.get_ft_info(id).await?;
		let global = response.global.unwrap();
		let ft = response.result;

		if ft.ticker != self.ticker {
			Err(anyhow::anyhow!("ticker mismatch"))?;
		}
		if ft.subtype != "decentralized" {
			Err(anyhow::anyhow!("not decentralized"))?;
		}
		if ft.mint_height > global.height + 1 {
			Err(anyhow::anyhow!("mint height mismatch"))?;
		}
		if ft.mint_amount == 0 || ft.mint_amount >= 100_000_000 {
			Err(anyhow::anyhow!("mint amount mismatch"))?;
		}
		if ft.dft_info.mint_count >= ft.max_mints {
			Err(anyhow::anyhow!("max mints reached"))?;
		}

		let satsbyte = util::query_fee().await?.min(self.max_fee) + 5;
		let additional_outputs = vec![TxOut {
			value: Amount::from_sat(ft.mint_amount),
			script_pubkey: wallet.stash.address.script_pubkey(),
		}];
		let payload = PayloadWrapper {
			args: Payload {
				bitworkc: ft.mint_bitworkc.clone(),
				mint_ticker: ft.ticker.clone(),
				nonce: rand::thread_rng().gen_range(1..10_000_000),
				time: util::now(),
			},
		};
		let payload_encoded = util::cbor(&payload)?;
		let reval_script =
			util::build_reval_script(&wallet.funding.x_only_public, "dmt", &payload_encoded);
		let hashscript = reval_script.tapscript_leaf_hash();
		let script_p2tr = Address::p2tr(
			&Default::default(),
			wallet.funding.x_only_public,
			Some(hashscript.into()),
			// Currently, this only supports mainnet.
			Network::Bitcoin,
		);
		let fees = Self::fees_of(satsbyte, reval_script.as_bytes().len(), &additional_outputs);
		let funding_utxo = self
			.api
			.wait_until_utxo(
				wallet.funding.address.to_string().to_lowercase(),
				fees.commit_and_reveal_and_outputs,
			)
			.await?;

		Ok(Data {
			satsbyte,
			op_type: "dmt",
			bitwork_info_commit: ft.mint_bitworkc,
			dmt_option: DmpOption { mint_amount: ft.mint_amount, ticker: self.ticker.clone() },
			additional_outputs,
			script_p2tr,
			fees,
			funding_utxo,
		})
	}

	fn fees_of(satsbyte: u64, reveal_script_len: usize, additional_outputs: &[TxOut]) -> Fees {
		let satsbyte = satsbyte as f64;
		let commit = {
			(satsbyte * (Self::BASE_BYTES + Self::INPUT_BYTES_BASE + Self::OUTPUT_BYTES_BASE))
				.ceil() as u64
		};
		let reveal = {
			let compact_input_bytes = if reveal_script_len <= 252 {
				1.
			} else if reveal_script_len <= 0xFFFF {
				3.
			} else if reveal_script_len <= 0xFFFFFFFF {
				5.
			} else {
				9.
			};

			(satsbyte
				* (Self::BASE_BYTES
						+ Self::REVEAL_INPUT_BYTES_BASE
						+ (compact_input_bytes + reveal_script_len as f64) / 4.
						// + utxos.len() as f64 * Self::INPUT_BYTES_BASE
						+ additional_outputs.len() as f64 * Self::OUTPUT_BYTES_BASE))
				.ceil() as u64
		};
		let outputs = additional_outputs.iter().map(|o| o.value.to_sat()).sum::<u64>();
		let commit_and_reveal = commit + reveal;
		let commit_and_reveal_and_outputs = commit_and_reveal + outputs;

		// While satsbyte at `150`.
		// Fees {
		// 	commit: 16650,
		// 	commit_and_reveal: 38700,
		// 	commit_and_reveal_and_outputs: 58700,
		// 	reveal: 22050,
		// 	reveal_and_outputs: 42050,
		// };
		Fees {
			commit,
			commit_and_reveal,
			commit_and_reveal_and_outputs,
			reveal,
			reveal_and_outputs: reveal + outputs,
		}
	}

	fn sequence_chunks() -> Vec<Range<u32>> {
		let step = (Sequence::MAX.0 as f64 / num_cpus::get() as f64).ceil() as u32;
		let mut ranges = Vec::new();
		let mut start = 0;

		while start < Sequence::MAX.0 {
			let end = start.checked_add(step).unwrap_or(Sequence::MAX.0);

			ranges.push(start..end);

			start = end;
		}

		ranges
	}
}
#[derive(Debug)]
struct MinerBuilder<'a> {
	electrumx: &'a str,
	wallet_dir: &'a Path,
	ticker: &'a str,
	max_fee: u64,
}
impl<'a> MinerBuilder<'a> {
	fn build(self) -> Result<Miner> {
		let api = ElectrumXBuilder::default().base_uri(self.electrumx).build().unwrap();
		let wallets = RawWallet::load_wallets(self.wallet_dir)
			.into_iter()
			.map(Wallet::try_from)
			.collect::<Result<_>>()?;

		Ok(Miner { api, wallets, ticker: self.ticker.into(), max_fee: self.max_fee })
	}
}

#[derive(Debug)]
struct Wallet {
	stash: Key,
	funding: Key,
}
impl TryFrom<RawWallet> for Wallet {
	type Error = Error;

	fn try_from(v: RawWallet) -> Result<Self> {
		let stash_private_key = PrivateKey::from_wif(&v.stash.key.wif)?;
		let stash_x_only_public_key = util::x_only_public_key_of(&stash_private_key)?;
		let funding_private_key = PrivateKey::from_wif(&v.funding.wif)?;
		let funding_x_only_public_key = util::x_only_public_key_of(&funding_private_key)?;

		Ok(Self {
			stash: Key {
				private: stash_private_key,
				x_only_public: stash_x_only_public_key,
				// Currently, this only supports mainnet.
				address: Address::from_str(&v.stash.key.address)
					.unwrap()
					.require_network(Network::Bitcoin)?,
			},
			funding: Key {
				private: funding_private_key,
				x_only_public: funding_x_only_public_key,
				// Currently, this only supports mainnet.
				address: Address::from_str(&v.funding.address)
					.unwrap()
					.require_network(Network::Bitcoin)?,
			},
		})
	}
}
#[derive(Debug)]
struct Key {
	private: PrivateKey,
	x_only_public: XOnlyPublicKey,
	address: Address,
}

#[derive(Debug, Serialize)]
pub struct PayloadWrapper {
	pub args: Payload,
}
#[derive(Debug, Serialize)]
pub struct Payload {
	pub bitworkc: String,
	pub mint_ticker: String,
	pub nonce: u64,
	pub time: u64,
}

// #[derive(Debug)]
// struct ScriptTree {
// 	output: Vec<u8>,
// }

// #[derive(Debug)]
// struct HashLockRedeem {
// 	output: Vec<u8>,
// 	redeem_version: u32,
// }

#[derive(Debug)]
struct Data {
	satsbyte: u64,
	op_type: &'static str,
	//  bitwork_info_commit: BitworkInfo,
	bitwork_info_commit: String,
	dmt_option: DmpOption,
	additional_outputs: Vec<TxOut>,
	script_p2tr: Address,
	fees: Fees,
	funding_utxo: Utxo,
}
// #[derive(Debug)]
//  struct BitworkInfo {
// 	 input_bitwork: String,
// 	 hex_bitwork: String,
// 	 prefix: String,
// }
#[derive(Debug)]
struct DmpOption {
	mint_amount: u64,
	ticker: String,
}
#[derive(Debug)]
struct Fees {
	commit: u64,
	commit_and_reveal: u64,
	commit_and_reveal_and_outputs: u64,
	reveal: u64,
	reveal_and_outputs: u64,
}

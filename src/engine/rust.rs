// std
use std::{
	ops::Range,
	path::Path,
	str::FromStr,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, Mutex,
	},
	thread::{self, JoinHandle},
};
// crates.io
use bitcoin::{
	absolute::LockTime,
	consensus::encode,
	hashes::Hash,
	key::TapTweak,
	psbt::Input,
	secp256k1::{All, Keypair, Message, Secp256k1, XOnlyPublicKey},
	sighash::{Prevouts, SighashCache},
	taproot::{LeafVersion, Signature, TaprootBuilder, TaprootSpendInfo},
	transaction::Version,
	Address, Amount, Network, OutPoint, Psbt, ScriptBuf, Sequence, TapSighashType, Transaction,
	TxIn, TxOut, Witness,
};
use serde::Serialize;
// atomicalsir
use crate::{
	electrumx::{r#type::Utxo, Api, ElectrumX, ElectrumXBuilder},
	prelude::*,
	util,
	wallet::Wallet as RawWallet,
};

pub async fn run(
	network: Network,
	electrumx: &str,
	wallet_dir: &Path,
	ticker: &str,
	max_fee: u64,
) -> Result<()> {
	let m = MinerBuilder { network, electrumx, wallet_dir, ticker, max_fee }.build()?;

	#[allow(clippy::never_loop)]
	loop {
		for w in &m.wallets {
			m.mine(w).await?;

			// Test only.
			// return Ok(());
		}
	}
}

#[derive(Debug)]
struct Miner {
	network: Network,
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
		let d = self.prepare_data(wallet).await?;

		tracing::info!("attempt to find a solution based on {d:#?}");

		let Data {
			secp,
			satsbyte,
			bitwork_info_commit,
			additional_outputs,
			reveal_script,
			reveal_spend_info,
			fees,
			funding_utxo,
		} = d.clone();
		let reveal_spk = ScriptBuf::new_p2tr(
			&secp,
			reveal_spend_info.internal_key(),
			reveal_spend_info.merkle_root(),
		);
		let funding_spk = wallet.funding.address.script_pubkey();
		let commit_input = vec![TxIn {
			previous_output: OutPoint::new(funding_utxo.txid.parse()?, funding_utxo.vout),
			..Default::default()
		}];
		let commit_output = {
			let spend = TxOut {
				value: Amount::from_sat(fees.reveal_and_outputs),
				script_pubkey: reveal_spk.clone(),
			};
			let refund = {
				let r = funding_utxo.value.saturating_sub(fees.reveal_and_outputs).saturating_sub(
					fees.commit + (Self::OUTPUT_BYTES_BASE * satsbyte as f64).floor() as u64,
				);

				if r > 0 {
					Some(TxOut { value: Amount::from_sat(r), script_pubkey: funding_spk.clone() })
				} else {
					None
				}
			};

			if let Some(r) = refund {
				vec![spend, r]
			} else {
				vec![spend]
			}
		};
		let commit_prevouts = [TxOut {
			value: Amount::from_sat(funding_utxo.value),
			script_pubkey: funding_spk.clone(),
		}];
		let mut ts = <Vec<JoinHandle<Result<()>>>>::new();
		let solution_found = Arc::new(AtomicBool::new(false));
		let maybe_commit_tx = Arc::new(Mutex::new(None));

		Self::sequence_ranges().into_iter().enumerate().for_each(|(i, r)| {
			tracing::info!("spawning thread {i} for sequence range {r:?}");

			let secp = secp.clone();
			let bitwork_info_commit = bitwork_info_commit.clone();
			let funding_kp = wallet.funding.pair.tap_tweak(&secp, None).to_inner();
			let funding_xpk = wallet.funding.x_only_public_key;
			let input = commit_input.clone();
			let output = commit_output.clone();
			let prevouts = commit_prevouts.clone();
			let hash_ty = TapSighashType::Default;
			let solution_found = solution_found.clone();
			let maybe_tx = maybe_commit_tx.clone();

			ts.push(thread::spawn(move || {
				for s in r {
					if solution_found.load(Ordering::Relaxed) {
						return Ok(());
					}

					let mut psbt = Psbt::from_unsigned_tx(Transaction {
						version: Version::ONE,
						lock_time: LockTime::ZERO,
						input: {
							let mut i = input.clone();

							i[0].sequence = Sequence(s);

							i
						},
						output: output.clone(),
					})?;
					let tap_key_sig = {
						let h = SighashCache::new(&psbt.unsigned_tx)
							.taproot_key_spend_signature_hash(
								0,
								&Prevouts::All(&prevouts),
								hash_ty,
							)?;
						let m = Message::from_digest(h.to_byte_array());

						Signature { sig: secp.sign_schnorr(&m, &funding_kp), hash_ty }
					};

					psbt.inputs[0] = Input {
						witness_utxo: Some(prevouts[0].clone()),
						final_script_witness: {
							let mut w = Witness::new();

							w.push(tap_key_sig.to_vec());

							Some(w)
						},
						tap_key_sig: Some(tap_key_sig),
						tap_internal_key: Some(funding_xpk),
						..Default::default()
					};

					tracing::trace!("{psbt:#?}");

					let tx = psbt.extract_tx_unchecked_fee_rate();
					let txid = tx.txid();

					if txid.to_string().trim_start_matches("0x").starts_with(&bitwork_info_commit) {
						tracing::info!("solution found");
						tracing::info!("sequence {s}");
						tracing::info!("commit txid {txid}");
						tracing::info!("commit tx {tx:#?}");

						solution_found.store(true, Ordering::Relaxed);
						*maybe_tx.lock().unwrap() = Some(tx);

						return Ok(());
					}
				}

				Ok(())
			}));
		});

		for t in ts {
			t.join().unwrap()?;
		}

		// TODO: If no solution found.
		let commit_tx = maybe_commit_tx.lock().unwrap().take().unwrap();

		self.api.broadcast(encode::serialize_hex(&commit_tx)).await?;

		let commit_txid = commit_tx.txid();
		let commit_txid_ = self
			.api
			.wait_until_utxo(
				Address::from_script(&reveal_spk, self.network)?.to_string(),
				fees.reveal_and_outputs,
			)
			.await?
			.txid;

		assert_eq!(commit_txid, commit_txid_.parse()?);

		// TODO: bitworkr.
		let mut reveal_psbt = Psbt::from_unsigned_tx(Transaction {
			version: Version::ONE,
			lock_time: LockTime::ZERO,
			input: vec![TxIn {
				previous_output: OutPoint::new(commit_txid, 0),
				sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
				..Default::default()
			}],
			output: additional_outputs,
			// TODO: bitworkr.
			// {
			// 	let mut o = additional_outputs;
			//
			// 	o.push(TxOut { value: Amount::ZERO, script_pubkey: util::time_nonce_script() });
			//
			// 	o
			// },
		})?;
		let reveal_st = TapSighashType::SinglePlusAnyoneCanPay;
		let reveal_tks = {
			let lh = reveal_script.tapscript_leaf_hash();
			let h = SighashCache::new(&reveal_psbt.unsigned_tx)
				.taproot_script_spend_signature_hash(
					0,
					&Prevouts::One(0, commit_output[0].clone()),
					lh,
					reveal_st,
				)?;
			let m = Message::from_digest(h.to_byte_array());

			Signature { sig: secp.sign_schnorr(&m, &wallet.funding.pair), hash_ty: reveal_st }
		};

		reveal_psbt.inputs[0] = Input {
			// TODO: Check.
			witness_utxo: Some(commit_output[0].clone()),
			tap_internal_key: Some(reveal_spend_info.internal_key()),
			tap_merkle_root: reveal_spend_info.merkle_root(),
			final_script_witness: {
				let mut w = Witness::new();

				w.push(reveal_tks.to_vec());
				w.push(reveal_script.as_bytes());
				w.push(
					reveal_spend_info
						.control_block(&(reveal_script, LeafVersion::TapScript))
						.unwrap()
						.serialize(),
				);

				Some(w)
			},
			..Default::default()
		};

		let reveal_tx = reveal_psbt.extract_tx_unchecked_fee_rate();
		let reveal_txid = reveal_tx.txid();

		tracing::info!("reveal txid {reveal_txid}");
		tracing::info!("reveal tx {reveal_tx:#?}");

		self.api.broadcast(encode::serialize_hex(&reveal_tx)).await?;

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

		let secp = Secp256k1::new();
		let satsbyte = if self.network == Network::Bitcoin {
			util::query_fee().await?.min(self.max_fee) + 5
		} else {
			2
		};
		let additional_outputs = vec![TxOut {
			value: Amount::from_sat(ft.mint_amount),
			script_pubkey: wallet.stash.address.script_pubkey(),
		}];
		let payload = PayloadWrapper {
			args: {
				let (time, nonce) = util::time_nonce();

				Payload {
					bitworkc: ft.mint_bitworkc.clone(),
					mint_ticker: ft.ticker.clone(),
					nonce,
					time,
				}
			},
		};
		let payload_encoded = util::cbor(&payload)?;
		// TODO: More op types.
		let reveal_script =
			util::build_reval_script(&wallet.funding.x_only_public_key, "dmt", &payload_encoded);
		let reveal_spend_info = TaprootBuilder::new()
			.add_leaf(0, reveal_script.clone())?
			.finalize(&secp, wallet.funding.x_only_public_key)
			.unwrap();
		let fees = Self::fees_of(satsbyte, reveal_script.as_bytes().len(), &additional_outputs);
		let funding_utxo = self
			.api
			.wait_until_utxo(wallet.funding.address.to_string(), fees.commit_and_reveal_and_outputs)
			.await?;

		Ok(Data {
			secp,
			satsbyte,
			bitwork_info_commit: ft.mint_bitworkc,
			additional_outputs,
			reveal_script,
			reveal_spend_info,
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
			// commit_and_reveal,
			commit_and_reveal_and_outputs,
			// reveal,
			reveal_and_outputs: reveal + outputs,
		}
	}

	fn sequence_ranges() -> Vec<Range<u32>> {
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
	network: Network,
	electrumx: &'a str,
	wallet_dir: &'a Path,
	ticker: &'a str,
	max_fee: u64,
}
impl<'a> MinerBuilder<'a> {
	fn build(self) -> Result<Miner> {
		let api =
			ElectrumXBuilder::default().network(self.network).base_uri(self.electrumx).build()?;
		let wallets = RawWallet::load_wallets(self.wallet_dir)
			.into_iter()
			.map(|rw| Wallet::from_raw_wallet(rw, self.network))
			.collect::<Result<_>>()?;

		Ok(Miner {
			network: self.network,
			api,
			wallets,
			ticker: self.ticker.into(),
			max_fee: self.max_fee,
		})
	}
}

#[derive(Clone, Debug)]
struct Wallet {
	stash: Key,
	funding: Key,
}
impl Wallet {
	fn from_raw_wallet(raw_wallet: RawWallet, network: Network) -> Result<Self> {
		let s_p = util::keypair_from_wif(&raw_wallet.stash.key.wif)?;
		let f_p = util::keypair_from_wif(&raw_wallet.funding.wif)?;

		Ok(Self {
			stash: Key {
				pair: s_p,
				x_only_public_key: s_p.x_only_public_key().0,
				address: Address::from_str(&raw_wallet.stash.key.address)?
					.require_network(network)?,
			},
			funding: Key {
				pair: f_p,
				x_only_public_key: f_p.x_only_public_key().0,
				address: Address::from_str(&raw_wallet.funding.address)?
					.require_network(network)?,
			},
		})
	}
}

#[derive(Clone, Debug)]
struct Key {
	pair: Keypair,
	x_only_public_key: XOnlyPublicKey,
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

#[derive(Clone, Debug)]
struct Data {
	secp: Secp256k1<All>,
	satsbyte: u64,
	// bitwork_info_commit: BitworkInfo,
	bitwork_info_commit: String,
	additional_outputs: Vec<TxOut>,
	reveal_script: ScriptBuf,
	reveal_spend_info: TaprootSpendInfo,
	fees: Fees,
	funding_utxo: Utxo,
}
// #[derive(Clone, Debug)]
// struct BitworkInfo {
// 	input_bitwork: String,
// 	hex_bitwork: String,
// 	prefix: String,
// 	ext: u64,
// }
#[derive(Clone, Debug)]
struct Fees {
	commit: u64,
	// commit_and_reveal: u64,
	commit_and_reveal_and_outputs: u64,
	// reveal: u64,
	reveal_and_outputs: u64,
}

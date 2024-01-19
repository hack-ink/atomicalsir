// std
use std::{
	ops::Range,
	path::Path,
	str::FromStr,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, Mutex,
	},
	thread::{self, sleep, JoinHandle},
	time::{Duration, SystemTime, UNIX_EPOCH},
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
	const BROADCAST_SLEEP_SECONDS: u32 = 15;
	const INPUT_BYTES_BASE: f64 = 57.5;
	const MAX_BROADCAST_NUM: u32 = 20;
	const MAX_SEQUENCE: u32 = u32::MAX;
	// OP_RETURN size
	// 8-bytes value(roughly estimate), a one-byte script’s size
	// actual value size depends precisely on final nonce
	const OP_RETURN_BYTES: f64 = 21. + 8. + 1.;
	const OUTPUT_BYTES_BASE: f64 = 43.;
	const REVEAL_INPUT_BYTES_BASE: f64 = 66.;
	const SEQ_RANGE_BUCKET: u32 = 100_000_000;

	async fn mine(&self, wallet: &Wallet) -> Result<()> {
		let concurrency: u32 = num_cpus::get() as u32;
		let seq_range_per_revealer: u32 = Self::SEQ_RANGE_BUCKET / concurrency;

		let d = self.prepare_data(wallet).await?;

		tracing::info!("attempt to find a solution based on {d:#?}");
		tracing::info!("\nStarting commit stage mining now...\n");
		tracing::info!("Concurrency set to: {concurrency}");

		let Data {
			secp,
			satsbyte,
			bitworkc,
			bitworkr,
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
		let commit_hty = TapSighashType::Default;
		let mut ts = <Vec<JoinHandle<Result<()>>>>::new();
		let solution_found = Arc::new(AtomicBool::new(false));
		let maybe_commit_tx = Arc::new(Mutex::new(None));

		Self::sequence_ranges().into_iter().enumerate().for_each(|(i, r)| {
			tracing::info!("spawning commit worker thread {i} for sequence range {r:?}");

			let secp = secp.clone();
			let bitworkc = bitworkc.clone();
			let funding_kp = wallet.funding.pair.tap_tweak(&secp, None).to_inner();
			let funding_xpk = wallet.funding.x_only_public_key;
			let input = commit_input.clone();
			let output = commit_output.clone();
			let prevouts = commit_prevouts.clone();
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
								commit_hty,
							)?;
						let m = Message::from_digest(h.to_byte_array());

						Signature { sig: secp.sign_schnorr(&m, &funding_kp), hash_ty: commit_hty }
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

					if txid.to_string().trim_start_matches("0x").starts_with(&bitworkc) {
						tracing::info!("solution found for commit step");
						tracing::info!("commit sequence {s}");
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

		tracing::info!("\nStay calm and grab a drink! Commit workers have started mining...\n");
		for t in ts {
			t.join().unwrap()?;
		}

		// TODO: If no solution found.
		let commit_tx = maybe_commit_tx.lock().unwrap().take().unwrap();

		let commit_txid = commit_tx.txid();
		// tracing::info!("commit txid {}", commit_txid);
		tracing::info!("Broadcasting commit tx...");
		let raw_tx = encode::serialize_hex(&commit_tx);
		tracing::info!("raw tx: {}", &raw_tx);

		let mut attempts = 0;
		while attempts < Self::MAX_BROADCAST_NUM {
			if let Err(_) = self.api.broadcast(raw_tx.clone()).await {
				tracing::info!(
					"Network error, will retry to broadcast commit transaction in {} seconds...",
					Self::BROADCAST_SLEEP_SECONDS
				);
				sleep(Duration::from_secs(15));
				attempts += 1;
				continue;
			}
			break;
		}

		if attempts < Self::MAX_BROADCAST_NUM {
			tracing::info!("Successfully sent commit tx {commit_txid}");
		} else {
			tracing::info!("❌ Failed to send commit tx {commit_txid}");
			return Ok(());
		}

		tracing::info!("\nCommit workers have completed their tasks for the commit transaction.\n");

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

		// TODO: Move common code to a single function.
		let reveal_hty = TapSighashType::SinglePlusAnyoneCanPay;
		let reveal_lh = reveal_script.tapscript_leaf_hash();
		let reveal_tx = if let Some(bitworkr) = bitworkr {
			// exists bitworkr
			tracing::info!("\nStarting reveal stage mining now...\n");
			tracing::info!("Concurrency set to: {concurrency}");
			let psbt = Psbt::from_unsigned_tx(Transaction {
				version: Version::ONE,
				lock_time: LockTime::ZERO,
				input: vec![TxIn {
					previous_output: OutPoint::new(commit_txid, 0),
					sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
					..Default::default()
				}],
				output: additional_outputs,
			})?;
			let mut ts = <Vec<JoinHandle<Result<()>>>>::new();
			let solution_found = Arc::new(AtomicBool::new(false));
			let must_tx = Arc::new(Mutex::new(None));
			let solution_time = Arc::new(Mutex::<u64>::new(0));
			let solution_nonce = Arc::new(Mutex::<u32>::new(0));

			for i in 0..concurrency {
				tracing::info!("spawning reveal worker thread {i} for bitworkr");
				let secp = secp.clone();
				let bitworkr = bitworkr.clone();
				let funding_kp = wallet.funding.pair;
				let reveal_script = reveal_script.clone();
				let reveal_spend_info = reveal_spend_info.clone();
				let commit_output = commit_output.clone();
				let psbt = psbt.clone();
				let solution_found = solution_found.clone();
				let must_tx = must_tx.clone();
				let solution_time = solution_time.clone();
				let solution_nonce = solution_nonce.clone();

				ts.push(thread::spawn(move || {
					let mut seq_start = i * seq_range_per_revealer;
					let mut seq = seq_start;
					let mut seq_end = seq_start + seq_range_per_revealer - 1;
					if i == (concurrency - 1) {
						seq_end = Self::SEQ_RANGE_BUCKET - 1;
					}

					let mut unixtime =
						SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
					let mut nonces_generated: u32 = 0;

					loop {
						if seq > seq_end {
							if seq_end <= Self::MAX_SEQUENCE - Self::SEQ_RANGE_BUCKET {
								seq_start += Self::SEQ_RANGE_BUCKET;
								seq_end += Self::SEQ_RANGE_BUCKET;
								seq = seq_start;
							} else {
								// reveal worker thread stop mining w/o soluton found
								tracing::info!("reveal worker thread {i} traversed its range w/o solution found.");
							}
						}
						if seq % 10000 == 0 {
							tracing::trace!(
								"started reveal mining for sequence: {seq} - {}",
								(seq + 10000).min(seq_end)
							);
						}

						if solution_found.load(Ordering::Relaxed) {
							return Ok(());
						}

						if nonces_generated % 10000 == 0 {
							unixtime =
								SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
						}

						let mut psbt = psbt.clone();

						psbt.unsigned_tx.output.push(TxOut {
							value: Amount::ZERO,
							script_pubkey: util::solution_tm_nonce_script(unixtime, seq),
						});
						psbt.outputs.push(Default::default());

						let tap_key_sig = {
							let h = SighashCache::new(&psbt.unsigned_tx)
								.taproot_script_spend_signature_hash(
									0,
									&Prevouts::One(0, commit_output[0].clone()),
									reveal_lh,
									reveal_hty,
								)?;
							let m = Message::from_digest(h.to_byte_array());

							Signature {
								sig: secp.sign_schnorr(&m, &funding_kp),
								hash_ty: reveal_hty,
							}
						};

						psbt.inputs[0] = Input {
							// TODO: Check.
							witness_utxo: Some(commit_output[0].clone()),
							tap_internal_key: Some(reveal_spend_info.internal_key()),
							tap_merkle_root: reveal_spend_info.merkle_root(),
							final_script_witness: {
								let mut w = Witness::new();

								w.push(tap_key_sig.to_vec());
								w.push(reveal_script.as_bytes());
								w.push(
									reveal_spend_info
										.control_block(&(
											reveal_script.clone(),
											LeafVersion::TapScript,
										))
										.unwrap()
										.serialize(),
								);

								Some(w)
							},
							..Default::default()
						};

						let tx = psbt.extract_tx_unchecked_fee_rate();
						let txid = tx.txid();

						if txid.to_string().trim_start_matches("0x").starts_with(&bitworkr) {
							tracing::info!("solution found for reveal step");
							tracing::info!("reveal sequence {seq}");
							tracing::info!("solution at time: {unixtime}, solution nonce: {seq}");
							solution_found.store(true, Ordering::Relaxed);
							*must_tx.lock().unwrap() = Some(tx);
							*solution_time.lock().unwrap() = unixtime;
							*solution_nonce.lock().unwrap() = seq;

							tracing::info!("\nReveal workers have completed their tasks for the reveal transaction.\n");

							return Ok(());
						}

						seq += 1;
						nonces_generated += 1;
					}
				}));
			}

			tracing::info!(
				"\nDon't despair, it still takes some time! Reveal workers have started mining...\n"
			);
			for t in ts {
				t.join().unwrap()?;
			}

			let tx = must_tx.lock().unwrap().take().unwrap();

			tx
		} else {
			// No bitworkr
			let mut psbt = Psbt::from_unsigned_tx(Transaction {
				version: Version::ONE,
				lock_time: LockTime::ZERO,
				input: vec![TxIn {
					previous_output: OutPoint::new(commit_txid, 0),
					sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
					..Default::default()
				}],
				output: additional_outputs,
			})?;
			let tap_key_sig = {
				let h = SighashCache::new(&psbt.unsigned_tx).taproot_script_spend_signature_hash(
					0,
					&Prevouts::One(0, commit_output[0].clone()),
					reveal_lh,
					reveal_hty,
				)?;
				let m = Message::from_digest(h.to_byte_array());

				Signature { sig: secp.sign_schnorr(&m, &wallet.funding.pair), hash_ty: reveal_hty }
			};

			psbt.inputs[0] = Input {
				// TODO: Check.
				witness_utxo: Some(commit_output[0].clone()),
				tap_internal_key: Some(reveal_spend_info.internal_key()),
				tap_merkle_root: reveal_spend_info.merkle_root(),
				final_script_witness: {
					let mut w = Witness::new();

					w.push(tap_key_sig.to_vec());
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

			psbt.extract_tx_unchecked_fee_rate()
		};

		let reveal_txid = reveal_tx.txid();
		tracing::info!("reveal txid {}", reveal_txid);
		tracing::info!("reveal tx {reveal_tx:#?}");

		tracing::info!("Broadcasting reveal tx...");
		let raw_tx = encode::serialize_hex(&reveal_tx);
		tracing::info!("raw tx: {}", &raw_tx);
		let mut attempts = 0;
		while attempts < Self::MAX_BROADCAST_NUM {
			if let Err(_) = self.api.broadcast(raw_tx.clone()).await {
				tracing::info!(
					"Network error, will retry to broadcast reveal transaction in {} seconds...",
					Self::BROADCAST_SLEEP_SECONDS
				);
				sleep(Duration::from_secs(15));
				attempts += 1;
				continue;
			}
			break;
		}

		if attempts < Self::MAX_BROADCAST_NUM {
			tracing::info!("✅ Successfully sent reveal tx {reveal_txid}");
			tracing::info!("✨Congratulations! Mission completed.✨");
		} else {
			tracing::info!("❌ Failed to send reveal tx {reveal_txid}");
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

		let secp = Secp256k1::new();
		let satsbyte = if self.network == Network::Bitcoin {
			(util::query_fee().await? + 5).min(self.max_fee)
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
				tracing::info!("payload time: {time}, payload nonce: {nonce}");

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
		let perform_bitworkr = if ft.mint_bitworkr.is_some() { true } else { false };
		let fees = Self::fees_of(
			satsbyte,
			reveal_script.as_bytes().len(),
			&additional_outputs,
			perform_bitworkr,
		);
		let funding_utxo = self
			.api
			.wait_until_utxo(wallet.funding.address.to_string(), fees.commit_and_reveal_and_outputs)
			.await?;

		Ok(Data {
			secp,
			satsbyte,
			bitworkc: ft.mint_bitworkc,
			bitworkr: ft.mint_bitworkr,
			additional_outputs,
			reveal_script,
			reveal_spend_info,
			fees,
			funding_utxo,
		})
	}

	fn fees_of(
		satsbyte: u64,
		reveal_script_len: usize,
		additional_outputs: &[TxOut],
		perform_bitworkr: bool,
	) -> Fees {
		let satsbyte = satsbyte as f64;
		let commit = {
			(satsbyte * (Self::BASE_BYTES + Self::INPUT_BYTES_BASE + Self::OUTPUT_BYTES_BASE))
				.ceil() as u64
		};
		let op_return_size_bytes = if perform_bitworkr { Self::OP_RETURN_BYTES } else { 0. };
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
                        + op_return_size_bytes
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
		let concurrency: u32 = num_cpus::get() as u32;
		let step = (Sequence::MAX.0 as f64 / concurrency as f64).ceil() as u32;
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

#[derive(Clone, Debug)]
struct Data {
	secp: Secp256k1<All>,
	satsbyte: u64,
	bitworkc: String,
	bitworkr: Option<String>,
	additional_outputs: Vec<TxOut>,
	reveal_script: ScriptBuf,
	reveal_spend_info: TaprootSpendInfo,
	fees: Fees,
	funding_utxo: Utxo,
}
#[derive(Clone, Debug)]
struct Fees {
	commit: u64,
	// commit_and_reveal: u64,
	commit_and_reveal_and_outputs: u64,
	// reveal: u64,
	reveal_and_outputs: u64,
}

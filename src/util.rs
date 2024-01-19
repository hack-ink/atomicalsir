// std
use std::{
	future::Future,
	time::{Duration, SystemTime, UNIX_EPOCH},
};
// crates.io
use bitcoin::{
	opcodes::{
		all::{OP_CHECKSIG, OP_ENDIF, OP_IF, OP_RETURN},
		OP_0,
	},
	script::PushBytes,
	secp256k1::Keypair,
	Address, PrivateKey, Script, ScriptBuf, XOnlyPublicKey,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::time;
// atomicalsir
use crate::prelude::*;

pub async fn loop_fut<F, Fut, T>(function: F, target: &str) -> T
where
	F: Fn() -> Fut,
	Fut: Future<Output = Result<T>>,
{
	loop {
		if let Ok(f) = function().await {
			return f;
		}

		tracing::error!("failed to query {target}; retrying in 1 minute");

		time::sleep(Duration::from_secs(60)).await;
	}
}

pub async fn query_fee() -> Result<u64> {
	#[derive(Debug, Deserialize)]
	#[serde(rename_all = "camelCase")]
	struct FastestFee {
		fastest_fee: u64,
	}

	Ok(reqwest::get("https://mempool.space/api/v1/fees/recommended")
		.await?
		.json::<FastestFee>()
		.await?
		.fastest_fee)
}

pub fn kill_process(pid: u32) -> Result<()> {
	#[cfg(any(target_os = "linux", target_os = "macos"))]
	std::process::Command::new("kill").args(["-9", &pid.to_string()]).output()?;
	#[cfg(target_os = "windows")]
	std::process::Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).output()?;

	Ok(())
}

pub fn time_nonce() -> (u64, u64) {
	(
		SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
		rand::thread_rng().gen_range(1..10_000_000),
	)
}

pub fn cbor<T>(v: &T) -> Result<Vec<u8>>
where
	T: Serialize,
{
	let mut cbor = Vec::new();

	ciborium::into_writer(v, &mut cbor)?;

	Ok(cbor)
}
#[test]
fn cbor_should_work() {
	// atomicalsir
	use crate::engine::rust::{Payload, PayloadWrapper};

	assert_eq!(
		cbor(&PayloadWrapper {
			args: Payload {
				bitworkc: "aabbcc".into(),
				mint_ticker: "quark".into(),
				nonce: 9999999,
				time: 1704057427
			},
		}).unwrap(),
		array_bytes::hex2bytes_unchecked("a16461726773a468626974776f726b63666161626263636b6d696e745f7469636b657265717561726b656e6f6e63651a0098967f6474696d651a6591da53")
	);
}

pub fn keypair_from_wif<S>(wif: S) -> Result<Keypair>
where
	S: AsRef<str>,
{
	Ok(Keypair::from_secret_key(&Default::default(), &PrivateKey::from_wif(wif.as_ref())?.inner))
}

pub fn build_reval_script(
	x_only_public_key: &XOnlyPublicKey,
	op_type: &str,
	payload: &[u8],
) -> ScriptBuf {
	// format!(
	// 	"{} OP_CHECKSIG OP_0 OP_IF {} {} {} OP_ENDIF",
	// 	&private_key.public_key(&Default::default()).to_string()[2..],
	// 	array_bytes::bytes2hex("", "atom"),
	// 	array_bytes::bytes2hex("", op_type),
	// 	payload.chunks(520).map(|c| array_bytes::bytes2hex("", c)).collect::<Vec<_>>().join(" ")
	// )
	let b = Script::builder()
		.push_x_only_key(x_only_public_key)
		.push_opcode(OP_CHECKSIG)
		.push_opcode(OP_0)
		.push_opcode(OP_IF)
		.push_slice(<&PushBytes>::try_from("atom".as_bytes()).unwrap())
		.push_slice(<&PushBytes>::try_from(op_type.as_bytes()).unwrap());

	payload
		.chunks(520)
		.fold(b, |b, c| b.push_slice(<&PushBytes>::try_from(c).unwrap()))
		.push_opcode(OP_ENDIF)
		.into_script()
}
#[test]
fn build_reval_script_should_work() {
	// atomicalsir
	use crate::engine::rust::{Payload, PayloadWrapper};

	// assert_eq!(
	// 	build_reval_script(
	// 		&keypair_from_wif("L4VgnxVoaPRaptd4yW19wwd7v9dzJvQn478AKwucbaQifPFBacrp").unwrap().
	// x_only_public_key().0, 		"dmt",
	// 		&cbor(&PayloadWrapper {
	// 			args: Payload {
	// 				bitworkc: "aabbcc".into(),
	// 				mint_ticker: "quark".into(),
	// 				nonce: 9999999,
	// 				time: 1704057427
	// 			},
	// 		}).unwrap()
	// 	),
	// 	"7e41d0ce6e41328e17ec13076603fc9d7a1d41fb1b497af09cdfbf9b648f7480 OP_CHECKSIG OP_0 OP_IF 61746f6d 646d74 a16461726773a468626974776f726b63666161626263636b6d696e745f7469636b657265717561726b656e6f6e63651a0098967f6474696d651a6591da53 OP_ENDIF"
	// );
	assert_eq!(
		array_bytes::bytes2hex(
			"",
			build_reval_script(
				&keypair_from_wif("L4VgnxVoaPRaptd4yW19wwd7v9dzJvQn478AKwucbaQifPFBacrp").unwrap().x_only_public_key().0,
				"dmt",
				&cbor(&PayloadWrapper {
					args: Payload {
						bitworkc: "aabbcc".into(),
						mint_ticker: "quark".into(),
						nonce: 9999999,
						time: 1704057427
					},
				})
				.unwrap()
			),
		),
		"207e41d0ce6e41328e17ec13076603fc9d7a1d41fb1b497af09cdfbf9b648f7480ac00630461746f6d03646d743ea16461726773a468626974776f726b63666161626263636b6d696e745f7469636b657265717561726b656e6f6e63651a0098967f6474696d651a6591da5368"
	);
}

pub fn solution_tm_nonce_script(time: u64, nonce: u32) -> ScriptBuf {
	Script::builder()
		.push_opcode(OP_RETURN)
		.push_slice(<&PushBytes>::try_from(format!("{time}:{nonce}").as_bytes()).unwrap())
		.into_script()
}

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

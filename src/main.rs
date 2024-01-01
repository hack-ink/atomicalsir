#![feature(async_closure)]

// std
use std::{
	fs::{self, File, OpenOptions},
	future::Future,
	io::{BufRead, BufReader, Write},
	path::{Path, PathBuf},
	process::{Command, Stdio},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	thread,
	time::Duration,
};
// crates.io
use anyhow::{Error, Result};
use clap::{
	builder::{
		styling::{AnsiColor, Effects},
		Styles,
	},
	Parser, ValueEnum,
};
use serde::Deserialize;

#[tokio::main]
async fn main() -> Result<()> {
	color_eyre::install().unwrap();
	tracing_subscriber::fmt::init();

	let Cli { atomicals_js_dir, max_fee, no_unconfirmed_txs_check, stash, electrumx, strategy } =
		Cli::parse();
	let wallets = Wallet::load_wallets(&atomicals_js_dir.join("wallets"));
	let strategy = if no_unconfirmed_txs_check { Strategy::AverageFirst } else { strategy };

	tracing::info!("");
	tracing::info!("");

	strategy.log();

	if let Some(s) = &stash {
		tracing::info!("stash: {s}");
	} else {
		tracing::info!("stash: primary");
	}

	let mut sleep = true;

	loop {
		for w in &wallets {
			tracing::info!("");
			tracing::info!("");

			match strategy {
				Strategy::AverageFirst => {
					let i = if no_unconfirmed_txs_check {
						0
					} else {
						loop_query(
							async || query_unconfirmed_tx_count(&w.address).await,
							"unconfirmed transaction count",
						)
						.await
					};

					for _ in i..=12 {
						w.mine(max_fee, stash.as_deref(), electrumx.as_deref()).await?;

						sleep = false;
					}
				},
				Strategy::WalletFirst => 'inner: loop {
					if loop_query(
						async || { query_unconfirmed_tx_count(&w.address) }.await,
						"unconfirmed transaction count",
					)
					.await <= 12
					{
						w.mine(max_fee, stash.as_deref(), electrumx.as_deref()).await?;

						sleep = false;
					} else {
						break 'inner;
					}
				},
			}
		}

		if sleep {
			tracing::warn!("");
			tracing::warn!("");
			tracing::warn!(
				"all wallets have 12 or more unconfirmed transactions; sleeping for 1 minute"
			);
			thread::sleep(Duration::from_secs(60));

			sleep = true;
		}
	}
}

#[derive(Debug, Parser)]
#[command(
	version = concat!(
		env!("CARGO_PKG_VERSION"),
		"-",
		env!("VERGEN_GIT_SHA"),
		"-",
		env!("VERGEN_CARGO_TARGET_TRIPLE"),
	),
	about,
	rename_all = "kebab",
	styles = styles(),
)]
struct Cli {
	/// Path to the atomicals-js repository's folder.
	#[arg(value_name = "PATH")]
	atomicals_js_dir: PathBuf,
	/// Maximum acceptable fee.
	///
	/// This value will be passed to atomicals-js's `--satsbyte` flag if the current network's
	/// priority fee is larger than this value.
	#[arg(long, value_name = "VALUE", default_value_t = 150)]
	max_fee: u32,
	/// Disable the unconfirmed transaction count check.
	///
	/// This will disable the multi-wallet feature.
	#[arg(long, default_value_t = false)]
	no_unconfirmed_txs_check: bool,
	/// Specify the alias of the stash wallet.
	///
	/// The name should be able to find in `wallets/x.json`.
	/// And it will be passed to atomicals-js's `--initialowner` flag.
	#[arg(verbatim_doc_comment, long, value_name = "ALIAS")]
	stash: Option<String>,
	/// Specify the URI of the electrumx proxy electrumx.
	///
	/// Examples:
	/// - https://ep.atomicals.xyz/proxy
	/// - https://ep.atomicalmarket.com/proxy
	#[arg(long, value_name = "URI")]
	electrumx: Option<String>,
	/// Mining strategy.
	#[arg(value_enum, long, default_value_t = Strategy::default(), value_name = "STRATEGY")]
	strategy: Strategy,
}

#[derive(Debug)]
struct Wallet {
	path: PathBuf,
	address: String,
}
impl Wallet {
	fn load(path: PathBuf) -> Self {
		#[derive(Debug, Deserialize)]
		struct WalletJ {
			funding: Funding,
		}
		#[derive(Debug, Deserialize)]
		struct Funding {
			address: String,
		}

		let address = serde_json::from_reader::<_, WalletJ>(File::open(&path).unwrap())
			.unwrap()
			.funding
			.address;

		Self { path, address }
	}

	fn load_wallets(path: &Path) -> Vec<Wallet> {
		fs::read_dir(path)
			.ok()
			.map(|rd| {
				rd.filter_map(|e| {
					e.ok().and_then(|e| {
						if e.path().extension().map(|e| e == "json") == Some(true) {
							let w = Wallet::load(e.path());

							tracing::info!("loaded wallet {} from {}", w.address, w.path.display());

							Some(w)
						} else {
							None
						}
					})
				})
				.collect()
			})
			.unwrap_or_default()
	}

	async fn mine(&self, max_fee: u32, stash: Option<&str>, electrumx: Option<&str>) -> Result<()> {
		let fee = loop_query(query_fee, "fee").await;

		tracing::info!("current priority fee: {fee} sat/vB");

		// Add 5 more to increase the speed.
		let fee = (fee + 5).min(max_fee).to_string();

		tracing::info!("selected: {fee} sat/vB");

		execute(
			"yarn",
			&["cli", "mint-dft", "quark", "--satsbyte", fee.as_str(), "--disablechalk"],
			&self.path,
			stash,
			electrumx,
		)?;

		Ok(())
	}
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Strategy {
	AverageFirst,
	WalletFirst,
}
impl Strategy {
	fn log(&self) {
		match self {
			Self::AverageFirst => tracing::info!("strategy: average first"),
			Self::WalletFirst => tracing::info!("strategy: wallet first"),
		}
	}
}
impl Default for Strategy {
	fn default() -> Self {
		Self::WalletFirst
	}
}

fn styles() -> Styles {
	Styles::styled()
		.header(AnsiColor::Red.on_default() | Effects::BOLD)
		.usage(AnsiColor::Red.on_default() | Effects::BOLD)
		.literal(AnsiColor::Blue.on_default() | Effects::BOLD)
		.placeholder(AnsiColor::Green.on_default())
}

fn execute(
	command: &str,
	args: &[&str],
	wallet_file: &Path,
	stash: Option<&str>,
	electrumx: Option<&str>,
) -> Result<()> {
	let mut cmd = Command::new(command);
	let work_dir = wallet_file.parent().unwrap().parent().unwrap();
	let wallet_file = wallet_file.file_name().unwrap().to_str().unwrap();

	cmd.args(args)
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.env("WALLET_FILE", wallet_file)
		.current_dir(work_dir);

	if let Some(s) = stash {
		cmd.args(["--initialowner", s]);
	}
	if let Some(s) = electrumx {
		cmd.env("ELECTRUMX_PROXY_BASE_URL", s);
	}

	let mut child = cmd.spawn()?;
	let pid = child.id();
	let should_terminate = Arc::new(AtomicBool::new(false));
	let stdout_st = should_terminate.clone();
	let stdout_r = BufReader::new(child.stdout.take().unwrap());
	let mut stdout_l = OpenOptions::new().create(true).append(true).open("stdout.log")?;
	let stdout_t = thread::spawn(move || {
		for l in stdout_r.lines() {
			if stdout_st.load(Ordering::Relaxed) {
				break;
			}

			let l = l?;

			for e in [
				"too-long-mempool-chain, too many descendants",
				"insufficient fee, rejecting replacement",
				"502 Bad Gateway",
				"Request failed with status code 500",
			] {
				if l.contains(e) {
					tracing::warn!("{e}; killing process");

					#[allow(clippy::single_match)]
					match e {
						"502 Bad Gateway" | "Request failed with status code 500" =>
							tracing::warn!("it's best to set up your own electrumx proxy"),
						_ => (),
					}

					kill(pid)?;

					break;
				}
			}

			writeln!(stdout_l, "{l}")?;
		}

		Result::<_, Error>::Ok(())
	});
	let stderr_st = should_terminate.clone();
	let stderr_r = BufReader::new(child.stderr.take().unwrap());
	let mut stderr_l = OpenOptions::new().create(true).append(true).open("stderr.log")?;
	let stderr_t = thread::spawn(move || {
		for l in stderr_r.lines() {
			if stderr_st.load(Ordering::Relaxed) {
				break;
			}

			let l = l?;

			if l.contains("worker stopped with exit code 1") {
				tracing::warn!("worker stopped with exit code 1; killing process");

				kill(pid)?;

				break;
			}

			writeln!(stderr_l, "{l}")?;
		}

		Result::<_, Error>::Ok(())
	});

	let _ = child.wait();

	should_terminate.store(true, Ordering::Relaxed);
	stdout_t.join().unwrap()?;
	stderr_t.join().unwrap()?;

	Ok(())
}

async fn loop_query<F, Fut, T>(function: F, target: &str) -> T
where
	F: Fn() -> Fut,
	Fut: Future<Output = Result<T>>,
{
	loop {
		if let Ok(f) = function().await {
			return f;
		}

		tracing::warn!("failed to query {target}; retrying in 1 minute");

		thread::sleep(Duration::from_secs(60));
	}
}

async fn query_fee() -> Result<u32> {
	// #[derive(Debug, Deserialize)]
	// struct Satsbyte {
	// 	priority: u32,
	// }
	// let fee = reqwest::get("https://api.blockchain.info/mempool/fees")
	// 	.await?
	// 	.json::<Satsbyte>()
	// 	.await?
	// 	.priority + 5;
	#[derive(Debug, Deserialize)]
	#[serde(rename_all = "camelCase")]
	struct FastestFee {
		fastest_fee: u32,
	}

	Ok(reqwest::get("https://mempool.space/api/v1/fees/recommended")
		.await?
		.json::<FastestFee>()
		.await?
		.fastest_fee)
}

async fn query_unconfirmed_tx_count(address: &str) -> Result<u32> {
	#[derive(Debug, Deserialize)]
	struct Unspent {
		unconfirmed_n_tx: u32,
	}

	tracing::info!("fee: {address}");

	let unconfirmed_count = reqwest::get(format!(
		"https://api.blockcypher.com/v1/btc/main/addrs/{address}?unspentOnly=true",
	))
	.await?
	.json::<Unspent>()
	.await?
	.unconfirmed_n_tx;

	tracing::info!("unconfirmed transaction count: {unconfirmed_count}");

	Ok(unconfirmed_count)
}

fn kill(pid: u32) -> Result<()> {
	let pid = pid.to_string();

	#[cfg(any(target_os = "linux", target_os = "macos"))]
	Command::new("kill").args(["-9", &pid]).output()?;
	#[cfg(target_os = "windows")]
	Command::new("taskkill").args(["/F", "/PID", &pid]).output()?;

	Ok(())
}

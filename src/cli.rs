// std
use std::path::PathBuf;
// crates.io
use clap::{
	builder::{
		styling::{AnsiColor, Effects},
		Styles,
	},
	ArgGroup, Parser,
};
// atomicalsir
use crate::{engine::*, prelude::*};

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
#[clap(group = ArgGroup::new("engine").required(true))]
pub struct Cli {
	/// Use Rust native miner.
	///
	/// Need to provide a path to the atomicals-js repository's wallets directory.
	#[arg(long, group = "engine")]
	rust_engine: Option<PathBuf>,
	/// Use official atomicals-js miner.
	///
	/// Need to provide a path to the atomicals-js repository's directory.
	#[arg(long, value_name = "PATH", group = "engine")]
	js_engine: Option<PathBuf>,
	/// Maximum acceptable fee.
	///
	/// This value will be passed to atomicals-js's `--satsbyte` flag if the current network's
	/// priority fee is larger then this value.
	#[arg(long, value_name = "VALUE", default_value_t = 150)]
	max_fee: u64,
	/// Specify the URI of the electrumx proxy electrumx.
	///
	/// Examples:
	/// - https://ep.atomicals.xyz/proxy
	/// - https://ep.atomicalmarket.com/proxy
	#[arg(
		verbatim_doc_comment,
		long,
		value_name = "URI",
		default_value_t = String::from("https://ep.atomicals.xyz/proxy")
	)]
	electrumx: String,
	/// Ticker of the network to mine on.
	#[arg(long, value_name = "NAME")]
	ticker: String,
}
impl Cli {
	pub async fn run(self) -> Result<()> {
		let Cli { rust_engine, js_engine, max_fee, electrumx, ticker } = self;
		let ticker = ticker.to_lowercase();

		if let Some(d) = js_engine {
			js::run(&electrumx, &d, &ticker, max_fee).await?;
		} else if let Some(d) = rust_engine {
			rust::run(&electrumx, &d, &ticker, max_fee).await?;
		}

		Ok(())
	}
}

fn styles() -> Styles {
	Styles::styled()
		.header(AnsiColor::Red.on_default() | Effects::BOLD)
		.usage(AnsiColor::Red.on_default() | Effects::BOLD)
		.literal(AnsiColor::Blue.on_default() | Effects::BOLD)
		.placeholder(AnsiColor::Green.on_default())
}
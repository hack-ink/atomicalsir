// std
use std::{
	fs::OpenOptions,
	future::Future,
	io::{BufRead, BufReader, Write},
	path::Path,
	process::{Command, Stdio},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	thread,
	time::Duration,
};
// crates.io
use tokio::time;
// atomicalsir
use crate::{
	prelude::*,
	util::{self, FeeBound},
	wallet::Wallet,
};

pub async fn run(
	network: &str,
	fee_bound: &FeeBound,
	electrumx: &str,
	atomicals_js_dir: &Path,
	ticker: &str,
) -> Result<()> {
	let ws = Wallet::load_wallets(atomicals_js_dir.join("wallets"));

	loop {
		for w in &ws {
			tracing::info!("");
			tracing::info!("");

			w.mine(network, fee_bound, electrumx, ticker).await?;
		}
	}
}

impl Wallet {
	async fn mine(
		&self,
		network: &str,
		fee_bound: &FeeBound,
		electrumx: &str,
		ticker: &str,
	) -> Result<()> {
		tracing::info!("stash: {}", self.stash.key.address);
		tracing::info!("funding: {}", self.funding.address);

		let fee = if network == "livenet" {
			let f = loop_fut(util::query_fee, "fee").await;

			tracing::info!("current priority fee: {f} sat/vB");

			// Add 5 more to increase the speed.
			let f = fee_bound.apply(f + 5);

			tracing::info!("selected: {f} sat/vB");

			f
		} else {
			2
		};

		let dir = self.path.parent().unwrap().parent().unwrap();
		let wf = self.path.file_name().unwrap().to_str().unwrap();
		let mut cmd = Command::new("yarn");

		cmd.args([
			"cli",
			"mint-dft",
			ticker,
			"--satsbyte",
			&fee.to_string(),
			"--initialowner",
			&self.stash.alias,
			"--disablechalk",
		])
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.env("NETWORK", network)
		.env("ELECTRUMX_PROXY_BASE_URL", electrumx)
		.env("WALLET_FILE", wf)
		.current_dir(dir);

		execute(cmd)?;

		Ok(())
	}
}

fn execute(mut command: Command) -> Result<()> {
	let mut child = command.spawn()?;
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
					tracing::error!("{e}; killing process");

					#[allow(clippy::single_match)]
					match e {
						"502 Bad Gateway" | "Request failed with status code 500" =>
							tracing::error!("it's best to set up your own electrumx proxy"),
						_ => (),
					}

					kill_process(pid)?;

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
				tracing::error!("worker stopped with exit code 1; killing process");

				kill_process(pid)?;

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

async fn loop_fut<F, Fut, T>(function: F, target: &str) -> T
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

fn kill_process(pid: u32) -> Result<()> {
	#[cfg(any(target_os = "linux", target_os = "macos"))]
	std::process::Command::new("kill").args(["-9", &pid.to_string()]).output()?;
	#[cfg(target_os = "windows")]
	std::process::Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).output()?;

	Ok(())
}

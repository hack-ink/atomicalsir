// crates.io
use vergen::EmitBuilder;

fn main() {
	let mut emitter = EmitBuilder::builder();

	emitter.cargo_target_triple();

	// Disable the git version if installed from <crates.io>.
	if emitter.clone().git_sha(true).fail_on_error().emit().is_err() {
		println!("cargo:rustc-env=VERGEN_GIT_SHA=crates.io");

		emitter
	} else {
		*emitter.git_sha(true)
	}
	.emit()
	.unwrap();
}

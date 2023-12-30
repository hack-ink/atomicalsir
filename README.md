<div align="center">

# atomicalsir
### Atomicals mining manager.
[![License](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Checks](https://github.com/hack-ink/atomicalsir/actions/workflows/checks.yml/badge.svg?branch=main)](https://github.com/hack-ink/atomicalsir/actions/workflows/checks.yml)
[![Release](https://github.com/hack-ink/atomicalsir/actions/workflows/release.yml/badge.svg)](https://github.com/hack-ink/atomicalsir/actions/workflows/release.yml)
[![GitHub tag (latest by date)](https://img.shields.io/github/v/tag/hack-ink/atomicalsir)](https://github.com/hack-ink/atomicalsir/tags)
[![GitHub code lines](https://tokei.rs/b1/github/hack-ink/atomicalsir)](https://github.com/hack-ink/atomicalsir)
[![GitHub last commit](https://img.shields.io/github/last-commit/hack-ink/atomicalsir?color=red&style=plastic)](https://github.com/hack-ink/atomicalsir)

</div>

## Usage
```
Atomicals mining manager.

Usage: atomicalsir [OPTIONS] <PATH>

Arguments:
  <PATH>
          Path to the atomicals-js repository's folder

Options:
      --max-fee <VALUE>
          Maximum acceptable fee.

          This value will be passed to atomicals-js's `--satsbyte` flag if the current network's priority fee is larger then this value.

          [default: 150]

      --stash <ALIAS>
          Specify the alias of the stash wallet.

          The name should be able to find in `wallets/x.json`. And it will be passed to atomicals-js's `--initialowner` flag.

      --electrumx <URI>
          Specify the URI of the electrumx proxy electrumx.

          Examples: - https://ep.atomicals.xyz/proxy - https://ep.atomicalmarket.com/proxy

      --strategy <STRATEGY>
          Mining strategy

          [default: average-first]
          [possible values: average-first, wallet-first]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

### Installation
#### Install from `crates.io`
To install from `crates.io`, use the following command:
```sh
cargo install atomicalsir
```

#### Download the pre-built binary
You can download the pre-build binary from our [GitHub release](https://github.com/hack-ink/subalfred/releases)

#### Build from source code
To build from the source code, use the following commands:

```sh
git clone https://hack-ink/atomicalsir
cd atomicalsir
cargo build --release
```

#### Step-by-step setup
1. Follow the installation steps for [`atomicals-js`](https://github.com/atomicals/atomicals-js).
2. Follow the installation steps for [`atomicalsir`](#installation).
3. Run the following command: `atomicalsir --max-fee 150 <PATH to the atomicals-js folder>`

### Q&A
- **How to setup multi-wallet?**

  To set up a multi-wallet, place the `*.json` wallet files in the `atomicals-js/wallets` directory.

- **How to use one stash address in multi-wallet mining?**

  Add a wallet with a <NAME> under the `imported` field of your `atomicals-js/wallets/x.json` file.

  Then, run the command `atomicalsir --stash <NAME> ..`.

  You `atomicals-js/wallets/x.json` file should looks like below:
  ```json
  {
  	"phrase": "..",
  	"primary": {
  		"address": "..",
  		"path": "m/86'/0'/0'/0/0",
  		"WIF": ".."
  	},
  	"funding": {
  		"address": "..",
  		"path": "m/86'/0'/0'/1/0",
  		"WIF": ".."
  	},
  	"imported": {
  		"<NAME>": {
  			"address": "..",
  			"WIF": ".."
  		}
  	}
  }
  ```

- **What are the differences of `average-first` and `wallet-first` mining strategies?**
  - The `average-first` strategy mines 12 times for each wallet in one loop.
  - The `wallet-first` strategy mines indefinitely, switching wallets until the current wallet has more than 12 unconfirmed transactions.

## Future plan
- [ ] Implement wallet balance detection.
- [ ] Implement a mining worker in pure Rust.

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

When creating and open-sourcing this project, I hoped to support those who love the Atomicals protocol and promote community development.

Thanks to the language features of Rust, before GPU mining algorithms were introduced, atomicalsir's hash computation speed surpassed all other tools. We are glad that I could help those in need and received some Stars.

However, during a piece of test code, a BTC mainnet private key was inadvertently used, leading to an incorrect transfer of some $QUARK worth about $10,000 into what should have been only for testing purposes. Although the amount is not large, the act of reading the code and then secretly transferring assets is utterly intolerable.

This project is entirely non-profit and born out of a love for open source and technology.

Stolen address: `bc1p2y56txyct5xfws8la3cdng802p96trmx6rj07pc094avxllmuryql08fgv`

The above is merely personal reflection. The fundamental reason is my own mistake, but I am deeply disappointed with this atmosphere.

Lastly, if a technology can't protect the bad guys, then it can't protect the good guys either. ;）


## Usage
```
Atomicals mining manager.

Usage: atomicalsir [OPTIONS] --fee-bound <MIN,MAX> --ticker <NAME> <--rust-engine <RUST_ENGINE>|--js-engine <PATH>>

Options:
      --rust-engine <RUST_ENGINE>
          Use Rust native miner.

          Need to provide a path to the atomicals-js repository's wallets directory.

      --js-engine <PATH>
          Use official atomicals-js miner.

          Need to provide a path to the atomicals-js repository's directory.

      --thread <NUM>
          Thread count.

          This adjusts the number of threads utilized by the Rust engine miner.

          [default: 16]

      --network <NETWORK>
          Network type

          [default: mainnet]
          [possible values: mainnet, testnet]

      --fee-bound <MIN,MAX>
          Set the fee rate range to sat/vB

      --electrumx <URI>
          Specify the URI of the electrumx.

          [default: https://ep.atomicals.xyz/proxy]

      --ticker <NAME>
          Ticker of the network to mine on

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

### Warning
The Rust mining engine is not fully tested; use at your own risk.

#### Example
```sh
RUST_LOG=atomicalsir=debug cargo r -r -- --rust-engine .maintain/atomicals-js/wallets --network testnet --fee-bound 50,150 --electrumx https://eptestnet.atomicals.xyz/proxy --ticker atomicalsir4
```

#### [Bitcoin testnet result](https://mempool.space/testnet/tx/aabbcc683171c11c3513f88f0c601e2657982e07d4e9259c8cfa4d909eb397bc)
```
yarn cli get 1809bfb8f69a1de200b5a5b2e96afef9f7417ee0772363c7a7cb6591eb1a9b8bi0
yarn run v1.22.21
$ node dist/cli.js get 1809bfb8f69a1de200b5a5b2e96afef9f7417ee0772363c7a7cb6591eb1a9b8bi0
walletPath ./wallets/wallet.json
{
  "global": {
    "coin": "BitcoinTestnet",
    "network": "testnet",
    "height": 2571723,
    "block_tip": "000000000000000ac41f551432d9f4bbc9a0fe5bbff561a32731ab1dd4da898b",
    "server_time": "2024-01-08T19:47:27.900342",
    "atomicals_block_tip": "0d40515d97e5b5185d055da4a3c8cb79a16f4e42dafda39b63729cc6698245ea",
    "atomical_count": 744,
    "atomicals_block_hashes": {
      "2571714": "b3b94255efd2d94ba70ec77cb298cb34e9b5a70a3e23ca04f12cabad142e0b71",
      "2571715": "caac375454e2006bd771d32df1b3918031f43397d61846b46764a1bb474d4394",
      "2571716": "48549f761486125b5be154b0f57bdf121e3c51f2703c8817188d6334fdac5a12",
      "2571717": "e01485011c133fb0933c0943efa87e0d566cd2d45724657eced1c106d8c1f7da",
      "2571718": "8e0fe1f6323f8b78bcdef48ca73733464412259876dfef0d32b409d4b02716f5",
      "2571719": "281f004f508ac251aa383746c606bb99d666cb23891390eb0b7154ad88fe9ba5",
      "2571720": "21920c557f6c5a30acee33f42b6d38b49cd9b1fc355388d21672bf73b67bb8a4",
      "2571721": "4395aeda9c362ffc9624abb886bcd911f47ed76608643d012fd941c127ea3354",
      "2571722": "c2d92352cf6e11d89c382fec2605351355d1e16a80f7b98f21af2ac6663d8b45",
      "2571723": "0d40515d97e5b5185d055da4a3c8cb79a16f4e42dafda39b63729cc6698245ea"
    }
  },
  "result": {
    "atomical_id": "1809bfb8f69a1de200b5a5b2e96afef9f7417ee0772363c7a7cb6591eb1a9b8bi0",
    "atomical_number": 743,
    "atomical_ref": "304vze7pk8ey405nmpsejtqyz7vm2zq0ewhp7hx7sdjs3trtke5gi0",
    "type": "FT",
    "confirmed": true,
    "mint_info": {
      "commit_txid": "1809bfb8f69a1de200b5a5b2e96afef9f7417ee0772363c7a7cb6591eb1a9b8b",
      "commit_index": 0,
      "commit_location": "1809bfb8f69a1de200b5a5b2e96afef9f7417ee0772363c7a7cb6591eb1a9b8bi0",
      "commit_tx_num": 68083224,
      "commit_height": 2571712,
      "reveal_location_txid": "8b40e05c14326ae372bf5f9f89e746446015376c25ab564d0b740468dd38af43",
      "reveal_location_index": 0,
      "reveal_location": "8b40e05c14326ae372bf5f9f89e746446015376c25ab564d0b740468dd38af43i0",
      "reveal_location_tx_num": 68083235,
      "reveal_location_height": 2571713,
      "reveal_location_header": "000000202198b97559f21c7288e03792fbf8e680ffed5a76b381b5a41b00000000000000acc3d6e71f6b523152e1aba92d3e802af5318f4f71557e5b392bc2e82be7a035e53f9c65874b6e19473878fa",
      "reveal_location_blockhash": "ba2dd3b0bb964ff88601e9367b8704ea9ba2fb54cc49e9253600000000000000",
      "reveal_location_scripthash": "6e36ddc9ec7b7906c84eb9687f61d0d397d19488dce7830af4e673c697940893",
      "reveal_location_script": "5120cef338caae67c64b2ec0865c7c997e74e0805f7c17fcad787b89acf5de28bbea",
      "reveal_location_value": 546,
      "args": {
        "time": 1704738727,
        "nonce": 6904474,
        "bitworkc": "1809",
        "max_mints": 499611,
        "mint_amount": 777,
        "mint_height": 0,
        "mint_bitworkc": "aabbcc",
        "request_ticker": "atomicalsir4"
      },
      "meta": {},
      "ctx": {},
      "$mint_bitworkc": "aabbcc",
      "$request_ticker": "atomicalsir4",
      "$bitwork": {
        "bitworkc": "1809",
        "bitworkr": null
      },
      "reveal_location_address": "tb1pemen3j4wvlryktkqsew8ext7wnsgqhmuzl7267rm3xk0th3gh04qr9wcec",
      "blockheader_info": {
        "version": 536870912,
        "prevHash": "000000000000001ba4b581b3765aedff80e6f8fb9237e088721cf25975b99821",
        "merkleRoot": "35a0e72be8c22b395b7e55714f8f31f52a803e2da9abe15231526b1fe7d6c3ac",
        "timestamp": 1704738789,
        "bits": 426658695,
        "nonce": 4202182727
      }
    },
    "subtype": "decentralized",
    "$max_supply": 388197747,
    "$mint_height": 0,
    "$mint_amount": 777,
    "$max_mints": 499611,
    "$mint_bitworkc": "aabbcc",
    "$bitwork": {
      "bitworkc": "1809",
      "bitworkr": null
    },
    "$ticker_candidates": [
      {
        "tx_num": 68083224,
        "atomical_id": "1809bfb8f69a1de200b5a5b2e96afef9f7417ee0772363c7a7cb6591eb1a9b8bi0",
        "txid": "1809bfb8f69a1de200b5a5b2e96afef9f7417ee0772363c7a7cb6591eb1a9b8b",
        "commit_height": 2571712,
        "reveal_location_height": 2571713
      }
    ],
    "$request_ticker_status": {
      "status": "verified",
      "verified_atomical_id": "1809bfb8f69a1de200b5a5b2e96afef9f7417ee0772363c7a7cb6591eb1a9b8bi0",
      "note": "Successfully verified and claimed ticker for current Atomical"
    },
    "$request_ticker": "atomicalsir4",
    "$ticker": "atomicalsir4",
    "mint_data": {
      "fields": {
        "args": {
          "time": 1704738727,
          "nonce": 6904474,
          "bitworkc": "1809",
          "max_mints": 499611,
          "mint_amount": 777,
          "mint_height": 0,
          "mint_bitworkc": "aabbcc",
          "request_ticker": "atomicalsir4"
        },
        "name": "Atomicalsir4",
        "image": "atom:btc:dat:115b57d4c16cf28a05f5b637a9dc422d576e10fca5a69e47eb8024d7abd13c18i0/image.png",
        "legal": {
          "terms": "Free"
        },
        "links": {
          "x": {
            "v": "https://x.com/AurevoirXavier"
          },
          "website": {
            "v": "https://github.com/hack-ink/atomicalsir"
          }
        }
      }
    }
  }
}

yarn cli wallets --balances --noqr
yarn run v1.22.21
$ node dist/cli.js wallets --balances --noqr
walletPath ./wallets/wallet.json
walletInfo result {
  success: true,
  data: {
    address: 'tb1pzvexmf6v30taky62fftyegejz8gtz3472e6rm4jmpswjjm0qq9hqe84j4h',
    scripthash: 'c714c3997bce0afc410c50f2c8fd84347cc156f0072203f9ca1a40f98baafe0b',
    atomicals_count: 1,
    atomicals_utxos: [ [Object], [Object], [Object] ],
    atomicals_balances: {
      '1809bfb8f69a1de200b5a5b2e96afef9f7417ee0772363c7a7cb6591eb1a9b8bi0': [Object]
    },
    total_confirmed: 2331,
    total_unconfirmed: 0,
    atomicals_confirmed: 2331,
    atomicals_unconfirmed: 0,
    regular_confirmed: 0,
    regular_unconfirmed: 0,
    regular_utxos: [],
    regular_utxo_count: 0,
    history: undefined
  }
}
```

### Installation
#### Install from `crates.io`
To install from `crates.io`, use the following command:
```sh
cargo install atomicalsir
```

#### Download the pre-built binary
You can download the pre-build binary from our [GitHub release](https://github.com/hack-ink/subalfred/releases)

#### Build from source code (requires the nightly Rust)
To build from the source code, use the following commands:

```sh
git clone https://hack-ink/atomicalsir
cd atomicalsir
cargo b -r
```

#### Step-by-step setup (js-engine)
1. Follow the installation steps for [`atomicals-js`](https://github.com/atomicals/atomicals-js#install).
2. Follow the installation steps for [`atomicalsir`](#installation).
3. Run the following command: `atomicalsir --js-engine <PATH to the atomicals-js folder> --fee-bound 50,150 --ticker quark`

#### Step-by-step setup (rust-engine)
1. Follow the installation steps for [`atomicalsir`](#installation).
2. Run the following command: `atomicalsir --rust-engine <PATH to the atomicals-js's wallets folder> --fee-bound 50,150 --ticker quark`

### Q&A
- **Where can I find the js-engine mining log?**

  You'll find the information in `stdout.log` and `stderr.log`, which are located in the current working directory.

- **How to setup multi-wallet?**

  To set up a multi-wallet, place the `*.json` wallet files in the `atomicals-js/wallets` directory.

- **How to use one stash address in multi-wallet mining?**

  Add a wallet with a `<NAME>` under the `imported` field of your `atomicals-js/wallets/x.json` file.

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
- [ ] Remove js-engine.
- [x] Implement wallet balance detection.
- [x] Implement mining worker in Rust.
- [ ] Implement GPU mining worker.

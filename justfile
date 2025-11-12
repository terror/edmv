set dotenv-load

export CARGO_MSG_LIMIT := '1'

default:
	just --list

alias f := fmt
alias r := run
alias t := test

all: build test clippy fmt-check

[group: 'dev']
build:
  cargo build

[group: 'check']
check:
 cargo check

[group: 'check']
ci: test clippy forbid
  cargo fmt --all -- --check
  cargo update --locked --package edmv

[group: 'check']
clippy:
  cargo clippy --all --all-targets

[group: 'format']
fmt:
  cargo fmt

[group: 'format']
fmt-check:
  cargo fmt --all -- --check

[group: 'check']
forbid:
  ./bin/forbid

[group: 'dev']
install:
  cargo install -f edmv

[group: 'dev']
install-dev-deps:
  rustup install nightly
  rustup update nightly
  cargo install cargo-watch

[group: 'release']
publish:
  #!/usr/bin/env bash
  set -euxo pipefail
  rm -rf tmp/release
  gh repo clone https://github.com/terror/edmv tmp/release
  cd tmp/release
  VERSION=`sed -En 's/version[[:space:]]*=[[:space:]]*"([^"]+)"/\1/p' Cargo.toml | head -1`
  git tag -a $VERSION -m "Release $VERSION"
  git push origin $VERSION
  cargo publish
  cd ../..
  rm -rf tmp/release

[group: 'dev']
run *args:
  cargo run -- --{{args}}

[group: 'test']
test:
  cargo test --all --all-targets

[group: 'test']
test-release-workflow:
  -git tag -d test-release
  -git push origin :test-release
  git tag test-release
  git push origin test-release

[group: 'dev']
watch +COMMAND='test':
  cargo watch --clear --exec "{{COMMAND}}"

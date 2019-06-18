all: release

release:
	cargo +stable-i686-unknown-linux-gnu build --release

debug:
	cargo +stable-i686-unknown-linux-gnu build


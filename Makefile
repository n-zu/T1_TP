all: fmt test clippy

fmt:
	(cd client; cargo fmt)
	(cd server; cargo fmt)
	(cd common/packets; cargo fmt)

test:
	(cd client; cargo test)
	(cd server; cargo test)
	(cd common/packets; cargo test)

clippy:
	(cd client; cargo clippy)
	(cd server; cargo clippy)
	(cd common/packets; cargo clippy)

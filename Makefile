all: fmt test clippy

fmt:
	(cd client; cargo fmt)
	(cd server; cargo fmt)
	(cd common/packets; cargo fmt)
	(cd common/threadpool; cargo fmt)

test:
	(cd client; cargo test)
	(cd server; cargo test)
	(cd common/packets; cargo test)
	(cd common/threadpool; cargo test)

clippy:
	(cd client; cargo clippy)
	(cd server; cargo clippy)
	(cd common/packets; cargo clippy)
	(cd common/threadpool; cargo clippy)

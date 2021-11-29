TARGETS = client server common/packets common/threadpool

all: fmt test clippy;

%:
	$(foreach t, $(TARGETS), (cd $(t); cargo $@);)

run-server:
	(cd server; cargo run)

run-client:
	(cd client; cargo run)

run:
	(cd client; cargo run &)
	(cd server; cargo run)

TARGETS = http_final/common/config http_final/http_server http_final/thermometer

all: fmt test clippy;

%:
	$(foreach t, $(TARGETS), (cd $(t); cargo $@);)

run-server:
	(cd http_final/http_server; cargo build --release; cargo run --release)

run-thermometer:
	(cd http_final/thermometer; cargo build --release; cargo run --release)

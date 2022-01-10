TARGETS = final/common/config final/http_server final/thermometer
SFLAGS ?= -num-clients 100 -num-messages 1000 -timeout 20s -global-timeout 180s -rampup-delay 1s -rampup-size 10 -publisher-qos 1

all: fmt test clippy;

%:
	$(foreach t, $(TARGETS), (cd $(t); cargo $@);)

run-server:
	(cd server; cargo run --release config.txt)

run-client:
	(cd client; cargo build --release)
	(cd client; cargo run --release &)
	sleep 0.5

run: run-client run-server;

online-config:
	sed -i 's/^ip=.*/ip='$$(hostname -I | grep -Eo '^[^ ]+' | sed 's/\./\\\./g')'/' server/config.txt	

stress:
	(cd common/packets; cargo build --release)
	(cd common/threadpool; cargo build --release)
	@chmod +x server/stress.sh
	@(cd server; SFLAGS="$(SFLAGS)" ./stress.sh)

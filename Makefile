TARGETS = client server common/packets common/threadpool
SFLAGS ?= -num-clients 100 -num-messages 1000 -timeout 20s -global-timeout 180s -rampup-delay 1s -rampup-size 10 -publisher-qos 1

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

online-config:
	sed -i 's/^ip=.*/ip='$$(hostname -I | grep -Eo '^[^ ]+' | sed 's/\./\\\./g')'/' server/config.txt	

stress:
	@chmod +x server/stress.sh
	@(cd server; SFLAGS="$(SFLAGS)" ./stress.sh)

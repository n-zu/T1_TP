TARGETS = client server common/packets common/threadpool
SFLAGS ?= -num-clients 100 -num-messages 1000 -timeout 20s -global-timeout 180s -rampup-delay 1s -rampup-size 10

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
	@docker image load -i server/mqtt-stresser.tar
	@make online-config > /dev/null
	@rm -f /tmp/srv-input
	@mkfifo /tmp/srv-input
	@(cd server; cargo build --release)
	@printf "Ejecutando stress test con flags:\n\033[2;37m$(SFLAGS)\n"
	@(cd server; cat /tmp/srv-input | cargo run --release > /dev/null) & \
	docker run --rm inovex/mqtt-stresser -broker tcp://$$(hostname -I | grep -Eo '^[^ ]+'):1883 \
		-username fdelu -password fdelu \
		$(SFLAGS); \
	printf "\n" > /tmp/srv-input
	@rm -f /tmp/srv-input
	@git restore server/config.txt
	@sleep 0.5

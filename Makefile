TARGETS = client server common/packets common/threadpool

all: fmt test clippy;

%:
	$(foreach t, $(TARGETS), (cd $(t); cargo $@);)


BINARY := target/release/geotrace

.PHONY: build run clean setcap

build:
	cargo build --release

# Apply raw socket capability so the binary can run without sudo
setcap: build
	sudo setcap cap_net_raw+ep $(BINARY)

run: setcap
	$(BINARY) $(ARGS)

clean:
	cargo clean

BINARY := target/release/geotrace

.PHONY: build run clean

build:
	cargo build --release

run: build
	sudo $(BINARY) $(ARGS)

clean:
	cargo clean

.PHONY: build test test-core spike run-spike install logs log-file clean

build:
	cargo build --workspace

test:
	cargo test -p catlock-core

test-core:
	cargo test -p catlock-core

spike:
	cargo build -p catlock-spike

run-spike:
	cargo run -p catlock-spike -- --seconds 10

install:
	sudo install -m 755 target/debug/catlockd /usr/local/bin/catlockd
	sudo install -m 644 packaging/systemd/catlockd.service /etc/systemd/system/ 2>/dev/null || true
	sudo systemctl daemon-reload
	sudo systemctl restart catlockd

logs:
	journalctl -fu catlockd

log-file:
	tail -f /var/log/catlockd.log

clean:
	cargo clean
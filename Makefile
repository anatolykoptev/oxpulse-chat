.PHONY: build test lint deploy build-web docker

build:
	cargo build --release --locked

test:
	cargo test --workspace

lint:
	cargo fmt --all -- --check
	cargo clippy --workspace -- -D warnings

build-web:
	cd web && npm install && npm run build
	rm -rf assets/room
	cp -r web/build assets/room

docker:
	docker build -t oxpulse-chat .

deploy: build-web docker
	@echo "Built oxpulse-chat image"

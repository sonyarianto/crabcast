.PHONY: dev down logs server web build lint test fmt ci

dev: ## Run the full stack (server + web + icecast) in Docker
	docker compose -f docker/compose.yml up --build

down: ## Stop the full stack
	docker compose -f docker/compose.yml down

logs: ## Tail logs from all services
	docker compose -f docker/compose.yml logs -f

server: ## Run the Rust API locally (needs DATABASE_URL/BIND_ADDR env)
	cargo run --manifest-path server/Cargo.toml

web: ## Run the Vite app locally (needs the API on :8080)
	npm --prefix web run dev

build: ## Build the web app
	npm --prefix web run build

lint: ## Rust clippy + web eslint
	cargo clippy --manifest-path server/Cargo.toml --all-targets -- -D warnings
	npm --prefix web run lint

test: ## Run all tests
	cargo test --manifest-path server/Cargo.toml

fmt: ## Format all code (Rust + web)
	cargo fmt --manifest-path server/Cargo.toml
	npm --prefix web exec prettier -- --write .

ci: fmt lint test ## What CI runs (fmt check variant used in CI)
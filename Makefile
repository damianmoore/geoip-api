.PHONY: help build run clean docker-build docker-run docker-push test version

# Extract version from Cargo.toml
VERSION := $(shell grep '^version' Cargo.toml | head -n1 | cut -d '"' -f2)
IMAGE_NAME := geoip-api
REGISTRY ?= docker.io
REPO_NAME ?= $(USER)/$(IMAGE_NAME)
FULL_IMAGE := $(REGISTRY)/$(REPO_NAME):$(VERSION)
LATEST_IMAGE := $(REGISTRY)/$(REPO_NAME):latest

# Default target
help: ## Show this help message
	@echo "GeoIP API - Version $(VERSION)"
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

version: ## Show current version
	@echo $(VERSION)

build: ## Build the Rust binary locally (requires Rust toolchain)
	cargo build --release

test: ## Run tests
	cargo test

clean: ## Clean build artifacts
	cargo clean
	docker image prune -f

docker-build: ## Build Docker image with version tag
	docker build -t $(FULL_IMAGE) -t $(LATEST_IMAGE) .
	@echo "Built image: $(FULL_IMAGE)"
	@echo "Built image: $(LATEST_IMAGE)"

docker-run: ## Run Docker container with volume mount (foreground, auto-remove)
	docker run \
		--rm \
		--name geoip-api-$(VERSION) \
		-p 3000:80 \
		-v geoip-data:/data \
		$(FULL_IMAGE)

docker-run-dev: ## Run Docker container in foreground for development
	docker run --rm \
		--name geoip-api-dev \
		-p 80:80 \
		-v geoip-data:/data \
		$(FULL_IMAGE)

docker-stop: ## Stop and remove the running container
	-docker stop geoip-api-$(VERSION)
	-docker rm geoip-api-$(VERSION)

docker-logs: ## Show container logs
	docker logs -f geoip-api-$(VERSION)

docker-push: docker-build ## Push Docker image to registry
	docker push $(FULL_IMAGE)
	docker push $(LATEST_IMAGE)
	@echo "Pushed: $(FULL_IMAGE)"
	@echo "Pushed: $(LATEST_IMAGE)"

docker-multiarch: ## Build and push multi-architecture images
	docker buildx create --name geoip-builder --use || true
	docker buildx build --platform linux/amd64,linux/arm64 \
		-t $(FULL_IMAGE) \
		-t $(LATEST_IMAGE) \
		--push .
	@echo "Multi-arch images pushed:"
	@echo "  $(FULL_IMAGE)"
	@echo "  $(LATEST_IMAGE)"

install: ## Install Docker and create data volume
	docker volume create geoip-data || true
	@echo "Created Docker volume: geoip-data"

uninstall: docker-stop ## Remove container, images, and volume
	-docker rmi $(FULL_IMAGE) $(LATEST_IMAGE)
	-docker volume rm geoip-data
	@echo "Cleaned up all resources"

status: ## Show container status and image info
	@echo "=== Container Status ==="
	-docker ps --filter name=geoip-api
	@echo ""
	@echo "=== Images ==="
	-docker images | grep geoip-api
	@echo ""
	@echo "=== Volume ==="
	-docker volume inspect geoip-data 2>/dev/null | grep -E "(Name|Mountpoint)" || echo "Volume not found"

dev-setup: install ## Complete development setup
	@echo "Setting up development environment..."
	$(MAKE) docker-build
	@echo ""
	@echo "Development setup complete!"
	@echo "Run 'make docker-run-dev' to start the API server"

# Production deployment helpers
prod-deploy: docker-multiarch ## Deploy to production (builds and pushes multi-arch images)
	@echo "Production deployment complete"
	@echo "Pull image on target system with: docker pull $(FULL_IMAGE)"

# CI/CD helpers  
ci-build: ## CI build target (no cache, with build args)
	docker build --no-cache --progress=plain \
		-t $(FULL_IMAGE) \
		-t $(LATEST_IMAGE) \
		.

ci-test: ## CI test target
	docker run --rm $(FULL_IMAGE) --help
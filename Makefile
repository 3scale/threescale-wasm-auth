MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
PROJECT_PATH := $(patsubst %/,%,$(dir $(MKFILE_PATH)))
COMPOSEFILE := $(PROJECT_PATH)/compose/docker-compose.yaml
DOCKER_COMPOSE := docker-compose -f $(COMPOSEFILE)
OPEN_APP ?= xdg-open

.PHONY: release-extension
release-extension: export BUILD?=release
release-extension: ## Build a release WASM filter and docker image
	$(MAKE) build-extension

.PHONY: build-extension
build-extension: export IMAGE_VERSION?=latest
build-extension: build ## Build WASM filter and docker image
	$(MAKE) -C $(PROJECT_PATH)/servicemesh build

.PHONY: clean-extension
clean-extension: export IMAGE_VERSION?=latest
clean-extension: clean ## Clean WASM filter and docker image
	$(MAKE) -C $(PROJECT_PATH)/servicemesh clean

.PHONY: push-extension
push-extension: export IMAGE_VERSION?=latest
push-extension: ## Push WASM filter docker image
	$(MAKE) -C $(PROJECT_PATH)/servicemesh push

.PHONY: release
release: export BUILD?=release
release: ## Build release WASM filter
	$(MAKE) build

.PHONY: build
build: export TARGET?=wasm32-unknown-unknown
build: export BUILD?=debug
build: ## Build WASM filter
	if test "x$(BUILD)" = "xrelease"; then \
	  cargo build --target=$(TARGET) --release $(CARGO_EXTRA_ARGS) && \
	  wasm-snip -o $(PROJECT_PATH)/target/$(TARGET)/$(BUILD)/snipped.wasm $(PROJECT_PATH)/target/$(TARGET)/$(BUILD)/threescale_wasm_auth.wasm && \
		mv $(PROJECT_PATH)/target/$(TARGET)/$(BUILD)/threescale_wasm_auth.wasm $(PROJECT_PATH)/target/$(TARGET)/$(BUILD)/threescale_wasm_auth_cargo.wasm && \
		wasm-opt -O4 --dce -o $(PROJECT_PATH)/target/$(TARGET)/$(BUILD)/threescale_wasm_auth.wasm $(PROJECT_PATH)/target/$(TARGET)/$(BUILD)/snipped.wasm; \
	else \
	  cargo build --target=$(TARGET) $(CARGO_EXTRA_ARGS) ; \
	fi
	mkdir -p $(PROJECT_PATH)/compose/wasm
	cp $(PROJECT_PATH)/target/$(TARGET)/$(BUILD)/threescale_wasm_auth.wasm $(PROJECT_PATH)/compose/wasm/
	ln -f $(PROJECT_PATH)/target/$(TARGET)/$(BUILD)/threescale_wasm_auth.wasm $(PROJECT_PATH)/servicemesh/

clean: ## Clean WASM filter
	cargo clean
	rm -f $(PROJECT_PATH)/compose/wasm/threescale_wasm_auth.wasm
	rm -f $(PROJECT_PATH)/servicemesh/threescale_wasm_auth.wasm

.PHONY: doc
doc: ## Open project documentation
	cargo doc --open

.PHONY: istio-loglevel
istio-loglevel: ## Set Istio proxy log level (use LOG_LEVEL)
	$(MAKE) -C $(PROJECT_PATH)/servicemesh istio-loglevel

.PHONY: istio-logs
istio-logs: ## Stream Istio proxy logs
	$(MAKE) -C $(PROJECT_PATH)/servicemesh istio-logs

.PHONY: istio-deploy
istio-deploy: build ## Deploy extension to Istio
	@echo >&2 "************************************************************"
	@echo >&2 " You need to copy the WASM file to the cluster and mount it"
	@echo >&2 "  Check EnvoyFilter and annotations in the pod deployment"
	@echo >&2 "************************************************************"
	$(MAKE) -C $(PROJECT_PATH)/servicemesh istio-apply

.PHONY: istio-clean
istio-clean: ## Remove extension from Istio
	$(MAKE) -C $(PROJECT_PATH)/servicemesh istio-clean

.PHONY: ossm-deploy
ossm-deploy: build-extension ## Deploy extension to Openshift Service Mesh
	@echo >&2 "***************************************************************"
	@echo >&2 " You need to push the WASM container to an accessible registry"
	@echo >&2 "***************************************************************"
	$(MAKE) -C $(PROJECT_PATH)/servicemesh istio-apply

.PHONY: ossm-clean
ossm-clean: ## Remove extension from Openshift Service Mesh
	$(MAKE) -C $(PROJECT_PATH)/servicemesh istio-clean

.PHONY: up
up: ## Start docker-compose containers
	$(DOCKER_COMPOSE) up

.PHONY: stop
stop: ## Stop docker-compose containers
	$(DOCKER_COMPOSE) stop

.PHONY: status
status: ## Status of docker-compose containers
	$(DOCKER_COMPOSE) ps

.PHONY: top
top: ## Show runtime information about docker-compose containers
	$(DOCKER_COMPOSE) top

kill: ## Force-stop docker-compose containers
	$(DOCKER_COMPOSE) kill

.PHONY: down
down: ## Stop and remove containers and other docker-compose resources
	$(DOCKER_COMPOSE) down

.PHONY: shell
shell:
	$(DOCKER_COMPOSE) exec shell /bin/bash

.PHONY: proxy-info
proxy-info: export INDEX?=1
proxy-info: ## Obtain the local host address and port for a service (use SERVICE, PORT and optionally INDEX)
	$(DOCKER_COMPOSE) port --index $(INDEX) $(SERVICE) $(PORT)

.PHONY: proxy-url
proxy-url: export INDEX?=1
proxy-url: export SCHEME?=http
proxy-url: ## Obtain a URL for the given service (use SERVICE, PORT and optionally INDEX)
	$(DOCKER_COMPOSE) port --index $(INDEX) $(SERVICE) $(PORT)

.PHONY: proxy
proxy: export INDEX?=1
proxy: export SCHEME?=http
proxy: LOCALURL=$(shell $(DOCKER_COMPOSE) port --index $(INDEX) $(SERVICE) $(PORT))
proxy: ## Open service and port in a browser (same as proxy-info, but optionally define SCHEME and OPEN_APP)
	$(OPEN_APP) $(SCHEME)://$(LOCALURL)

.PHONY: sso
sso: export SERVICE=keycloak
sso: export PORT?=8080
sso: ## Open Keycloak SSO IDP
	$(MAKE) proxy

.PHONY: keycloak
keycloak: sso

.PHONY: ingress-helper
ingress-helper: export SERVICE?=ingress
ingress-helper: export PORT?=80
ingress-helper: export TARGET?=proxy-url
ingress-helper:
	$(MAKE) $(TARGET)

.PHONY: ingress-url
ingress-url: ## Show the ingress URL
	$(MAKE) ingress-helper

.PHONY: ingress-open
ingress-open: export TARGET?=proxy
ingress-open: ## Open the ingress URL
	$(MAKE) ingress-helper

.PHONY: ingress-admin-url
ingress-admin-url: export PORT?=8001
ingress-admin-url: ## Show the ingress admin URL
	$(MAKE) ingress-helper

.PHONY: ingress-admin-open
ingress-admin-open: export PORT?=8001
ingress-admin-open: export TARGET?=proxy
ingress-admin-open: ## Open the ingress admin URL
	$(MAKE) ingress-helper

.PHONY: curl
curl: export SCHEME?=http
curl: export SERVICE?=ingress
curl: export INDEX?=1
curl: export PORT?=80
curl: export HOST?=web.app
curl: export USER_KEY?=invalid-key
curl: export TARGET?=$$($(DOCKER_COMPOSE) port --index $(INDEX) $(SERVICE) $(PORT))
curl: ## Perform a request to a specific service (default ingress:80 with Host: web.app, please set USER_KEY)
	curl -vvv -H "Host: $(HOST)" -H "X-API-Key: $(USER_KEY)" "$(SCHEME)://$(TARGET)/$(SVC_PATH)"

.PHONY: curl-web.app
curl-web.app: export USER_KEY?=$(WEB_KEY)
curl-web.app: ## Perform a curl call to web.app (make sure to export secrets, ie. source ./env)
	$(MAKE) curl

.PHONY: curl-compose
curl-compose: curl-web.app ## Perform a curl call to the docker-compose cluster

.PHONY: curl-istio
curl-istio: export KUBECTL?=kubectl
curl-istio: export ISTIO_NS?=istio-system
curl-istio: export ISTIO_INGRESS?=istio-ingressgateway
curl-istio: export SVC_PATH?=productpage
curl-istio: export TARGET?=$(shell $(KUBECTL) -n $(ISTIO_NS) get service $(ISTIO_INGRESS) -o jsonpath='{.status.loadBalancer.ingress[0].ip}'):$(shell $(KUBECTL) -n $(ISTIO_NS) get service $(ISTIO_INGRESS) -o jsonpath='{.spec.ports[?(@.name=="http2")].port}')
curl-istio: ## Perform a curl call to your Istio Ingress
	$(MAKE) curl-web.app

.PHONY: curl-ossm
curl-ossm: export KUBECTL?=oc
curl-ossm: ## Perform a curl call to your Openshift Service Mesh or Maistra Ingress
	$(MAKE) curl-istio

# Check http://marmelab.com/blog/2016/02/29/auto-documented-makefile.html
.PHONY: help
help: ## Print this help
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

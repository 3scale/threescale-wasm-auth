PROJECT_PATH := $(patsubst %/,%,$(dir $(MKFILE_PATH)))
IMAGE_VERSION ?= latest
DOCKER ?= podman

.PHONY: build
build:
	$(DOCKER) build -t quay.io/3scale/threescale-wasm-auth:$(IMAGE_VERSION) -f $(PROJECT_PATH)/Dockerfile $(PROJECT_PATH)

.PHONY: clean
clean:
	-$(DOCKER) rmi quay.io/3scale/threescale-wasm-auth:$(IMAGE_VERSION)

.PHONY: push
push:
	$(DOCKER) push quay.io/3scale/threescale-wasm-auth:$(IMAGE_VERSION)

MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
PROJECT_PATH := $(patsubst %/,%,$(dir $(MKFILE_PATH)))
DOCKER ?= podman
KUBECTL ?= kubectl
ISTIOCTL ?= istioctl
ISTIO_NS ?= istio-system
BOOKINFO_NS ?= bookinfo
DEPLOY ?= deploy/productpage-v1

.PHONY: svcentry-apply
svcentry-apply:
	$(KUBECTL) -n $(BOOKINFO_NS) apply -f $(PROJECT_PATH)/crds/threescale-saas-svcentry.yaml

.PHONY: svcentry-clean
svcentry-clean:
	$(KUBECTL) -n $(BOOKINFO_NS) delete -f $(PROJECT_PATH)/crds/threescale-saas-svcentry.yaml

.PHONY: dstrule-apply
dstrule-apply:
	$(KUBECTL) -n $(BOOKINFO_NS) apply -f $(PROJECT_PATH)/crds/threescale-saas-dstrule.yaml

.PHONY: dstrule-clean
svcentry-clean:
	$(KUBECTL) -n $(BOOKINFO_NS) delete -f $(PROJECT_PATH)/crds/threescale-saas-dstrule.yaml

.PHONY: istio-apply
istio-apply: svcentry-apply dstrule-apply
	$(KUBECTL) -n $(BOOKINFO_NS) apply -f $(PROJECT_PATH)/crds/istio/

.PHONY: istio-clean
istio-clean: svcentry-clean dstrule-clean
	$(KUBECTL) -n $(BOOKINFO_NS) delete -f $(PROJECT_PATH)/crds/istio/

.PHONY: ossm-apply
ossm-clean: export KUBECTL?=oc
ossm-apply: svcentry-apply dstrule-apply
	$(KUBECTL) -n $(BOOKINFO_NS) apply -f $(PROJECT_PATH)/crds/ossm/

.PHONY: ossm-clean
ossm-clean: export KUBECTL?=oc
ossm-clean: svcentry-clean dstrule-apply
	$(KUBECTL) -n $(BOOKINFO_NS) delete -f $(PROJECT_PATH)/crds/ossm/

.PHONY: istio-loglevel
istio-loglevel: export LOG_LEVEL?=debug
istio-loglevel:
	$(ISTIOCTL) -i $(ISTIO_NS) -n $(BOOKINFO_NS) pc log $(DEPLOY) --level $(LOG_LEVEL)

.PHONY: istio-logs
istio-logs:
	$(KUBECTL) -n $(BOOKINFO_NS) logs $(DEPLOY) -c istio-proxy -f

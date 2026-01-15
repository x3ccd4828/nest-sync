REGISTRY := ghcr.io/x3ccd4828
IMAGE_NAME := $(shell cargo get package.name)
REPOSITORY := $(REGISTRY)/$(IMAGE_NAME)
mkfile_path := $(abspath $(lastword $(MAKEFILE_LIST)))
CONTAINER_NAME := $(notdir $(patsubst %/,%,$(dir $(mkfile_path))))
MANIFEST_NAME := $(IMAGE_NAME)-manifest
VERSION := $(shell cargo get package.version --pretty)

echo_vars:
	@echo $(REGISTRY)
	@echo $(REPOSITORY)
	@echo $(IMAGE_NAME)
	@echo $(MANIFEST_NAME)
	@echo $(CONTAINER_NAME)
	@echo $(VERSION)
	@echo "$(MANIFEST_NAME):$(VERSION)"

build:
	buildah manifest create "$(MANIFEST_NAME):$(VERSION)"
	buildah build --platform=linux/arm64/v8,linux/amd64 --manifest "$(MANIFEST_NAME):$(VERSION)" --tag $(IMAGE_NAME):$(VERSION) --file Containerfile ../.

push:
	# Push with explicit version tag (e.g., nest-sync:0.1.0) - permanent, immutable reference
	buildah manifest push --creds "$(REGISTRY_USER):$(REGISTRY_PASSWORD)" --all "$(MANIFEST_NAME):$(VERSION)" "docker://$(REPOSITORY):$(VERSION)"
	# Push as :latest tag - updates latest to point to this version
	buildah manifest push --creds "$(REGISTRY_USER):$(REGISTRY_PASSWORD)" --all "$(MANIFEST_NAME):$(VERSION)" "docker://$(REPOSITORY)"

push_without_creds: 
	# Push with explicit version tag (e.g., nest-sync:0.1.0) - permanent, immutable reference
	buildah manifest push --all "$(MANIFEST_NAME):$(VERSION)" "docker://$(REPOSITORY):$(VERSION)"
	# Push as :latest tag - updates latest to point to this version
	buildah manifest push --all "$(MANIFEST_NAME):$(VERSION)" "docker://$(REPOSITORY)" 

build_and_push: build push_without_creds

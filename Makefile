REGISTRY  := 192.168.0.66:5000
IMAGE     := claudia
VERSION   := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*= "\(.*\)"/\1/')
TAG       := $(REGISTRY)/$(IMAGE):$(VERSION)
TAG_LATEST:= $(REGISTRY)/$(IMAGE):latest

.PHONY: build push release run clean

build:
	docker build -t $(TAG) -t $(TAG_LATEST) .

push:
	docker push $(TAG)
	docker push $(TAG_LATEST)

release: build push

run:
	docker compose up -d

clean:
	docker compose down
	docker rmi $(TAG) $(TAG_LATEST) 2>/dev/null || true

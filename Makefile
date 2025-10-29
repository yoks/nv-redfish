#
# Build and test everything.
#

pwd := $(shell pwd)

maybe-lenovo-build = $(if $(wildcard $(pwd)/oem/lenovo/*.xml),cargo build --features oem-lenovo)

space := $(empty) $(empty)
comma :=,

# We cannot use --all-features because they depends on oem files that
# are not distributed by the repo.
all-std-features = accounts \
                   chassis \
                   systems \
                   update-service

ci-features-list := $(subst $(space),$(comma),$(all-std-features))

define build-and-test
	cargo build
	cargo test $1 -- --no-capture
	cargo clippy $1
	cargo build  $1
	cargo build --features oem-hpe,accounts
	$(maybe-lenovo-build)
	cargo build --features oem-hpe
	cargo build --features oem-nvidia
	cargo build --features oem-dell
	cargo build --features oem-ami
	cargo build --features accounts
	cargo build --features chassis
	cargo build --features systems
	cargo build --features update-service
	cargo build --features ""
	cargo doc $1
	cargo build

endef


all:
	$(call build-and-test,--all-features)

ci: rust-install
	$(call build-and-test,--features $(ci-features-list))

rust-install:
	rustup component add clippy



clean:
	rm -rf $(schema-dir)
	rm -rf target



ROOT_DIR ?= $(abspath ..)
BIN_DIR ?= $(ROOT_DIR)/bin
PROGRAM_DIR := $(BIN_DIR)/programs
LIB_DIR := $(BIN_DIR)/lib
LIBPUREC := $(LIB_DIR)/libpurec.a
LD_SCRIPT := $(CURDIR)/ld/userspace.ld

CARGO ?= cargo
CXX_USER ?= x86_64-elf-g++
CC_USER ?= x86_64-elf-gcc
CXX_OUT_DIR := $(CURDIR)/target/cxx/panel
PANEL_OBJECTS := $(CXX_OUT_DIR)/audio_panel.o $(CXX_OUT_DIR)/ring3.o
USER_CXXFLAGS := -g -O2 -ffreestanding -fno-stack-protector -fno-pic \
	-m64 -mno-red-zone -mcmodel=small -fno-exceptions -fno-rtti
USER_CFLAGS := -g -O2 -ffreestanding -fno-stack-protector -fno-pic \
	-m64 -mno-red-zone -mcmodel=small
TARGET := x86_64-unknown-none
PROFILE_FLAG := --release
TARGET_DIR := $(CURDIR)/target
OUT_DIR := $(TARGET_DIR)/$(TARGET)/release

BINS := hello desk_app desktop
VB_DIR := $(BIN_DIR)/video-backend
LIBVB := $(VB_DIR)/libvb.a
VERSION := $(shell cat VERSION)
PKGREL ?= 1
STAGED := $(BINS:%=$(OUT_DIR)/%)

export RUSTFLAGS := \
	-C no-redzone=yes \
	-C relocation-model=static \
	-C code-model=small \
	-C link-arg=-T$(LD_SCRIPT) \
	-C link-arg=$(CXX_OUT_DIR)/audio_panel.o \
	-C link-arg=$(CXX_OUT_DIR)/ring3.o \
	-C link-arg=-L$(VB_DIR) \
	-C link-arg=-l:libvb.a \
	-C link-arg=-L$(LIB_DIR) \
	-C link-arg=-l:libpurec.a

.PHONY: all libpurec libvb bins install package
all: install

libpurec:
	$(MAKE) -C $(ROOT_DIR)/src/libc

libvb:
	$(MAKE) -C $(ROOT_DIR)/video-backend

bins: libpurec libvb $(LD_SCRIPT) $(PANEL_OBJECTS)
	$(CARGO) build $(PROFILE_FLAG) --target $(TARGET)

$(CXX_OUT_DIR)/audio_panel.o: $(CURDIR)/cxx/panel/audio_panel.cpp $(CURDIR)/cxx/panel/audio_panel.h $(CURDIR)/cxx/panel/ring3.h
	@mkdir -p $(@D)
	$(CXX_USER) $(USER_CXXFLAGS) -I$(ROOT_DIR)/src/libc/include -c $< -o $@

$(CXX_OUT_DIR)/ring3.o: $(CURDIR)/cxx/panel/ring3.c $(CURDIR)/cxx/panel/ring3.h
	@mkdir -p $(@D)
	$(CC_USER) $(USER_CFLAGS) -I$(ROOT_DIR)/src/libc/include -I$(ROOT_DIR)/src -c $< -o $@

install: bins
	@mkdir -p $(PROGRAM_DIR)
	cp $(OUT_DIR)/hello $(PROGRAM_DIR)/hello-rs
	cp $(OUT_DIR)/desk_app $(PROGRAM_DIR)/desk-app
	cp $(OUT_DIR)/desktop $(PROGRAM_DIR)/desktop-rs
	@echo "staged: $(PROGRAM_DIR)/hello-rs $(PROGRAM_DIR)/desk-app $(PROGRAM_DIR)/desktop-rs"

PKG_OUT := $(BIN_DIR)/repo
PKG_FILE := $(PKG_OUT)/userspace-$(VERSION)-$(PKGREL)-x86_64.pkg.tar.gz
SIGN_KEY ?= $(HOME)/.config/purec-pkg/signing.pem
APP_ENTRIES := clock calc calendar

package: bins
	@mkdir -p $(PKG_OUT)
	rm -f $(PKG_OUT)/userspace-*.pkg.tar.gz*
	python3 $(ROOT_DIR)/mk/pkg/mkpkg.py --name userspace --version $(VERSION)-$(PKGREL) \
		--desc "PureC OS desktop: shell, panels and applications on video-backend" \
		--depend "video-backend>=0.2.0" -o $(PKG_OUT) \
		--file usr/bin/desktop-rs=$(OUT_DIR)/desktop \
		--file usr/bin/desk-app=$(OUT_DIR)/desk_app \
		--file usr/bin/hello-rs=$(OUT_DIR)/hello \
		$(foreach e,$(APP_ENTRIES),--file usr/share/applications/$(e).desktop=$(CURDIR)/desktop_entries/$(e).desktop) \
		--file usr/lib/init/system/desktop.service=$(CURDIR)/units/desktop.service \
		--file usr/lib/init/system/graphical.target.wants/desktop.service=$(CURDIR)/units/desktop.service
	if [ -f "$(SIGN_KEY)" ]; then python3 $(ROOT_DIR)/mk/pkg/pkgsign.py sign $(SIGN_KEY) $(PKG_FILE) $(PKG_FILE).sig; fi
	@ls -l $(PKG_OUT)/userspace-*

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

BINS := hello init apps_demo desktop
STAGED := $(BINS:%=$(OUT_DIR)/%)

export RUSTFLAGS := \
	-C no-redzone=yes \
	-C relocation-model=static \
	-C code-model=small \
	-C link-arg=-T$(LD_SCRIPT) \
	-C link-arg=$(CXX_OUT_DIR)/audio_panel.o \
	-C link-arg=$(CXX_OUT_DIR)/ring3.o \
	-C link-arg=-L$(LIB_DIR) \
	-C link-arg=-lpurec

.PHONY: all libpurec bins install
all: install

libpurec:
	$(MAKE) -C $(ROOT_DIR)/src/libc

bins: libpurec $(LD_SCRIPT) $(PANEL_OBJECTS)
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
	cp $(OUT_DIR)/init $(PROGRAM_DIR)/init-rs
	cp $(OUT_DIR)/apps_demo $(PROGRAM_DIR)/apps-demo-rs
	cp $(OUT_DIR)/desktop $(PROGRAM_DIR)/desktop-rs
	@echo "staged: $(PROGRAM_DIR)/hello-rs $(PROGRAM_DIR)/init-rs $(PROGRAM_DIR)/apps-demo-rs $(PROGRAM_DIR)/desktop-rs"
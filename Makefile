# PureC-OS-Userspace (Rust) — сборка Ring-3 программ поверх libpurec.a.
#
# In-tree (из корня ОС):  make -C userspace
# Standalone:             BIN_DIR=/path/to/bin make
#
# Rust-код линкуется с C-библиотекой: FFI-объявления в crates/purec
# резолвятся из $(LIB_DIR)/libpurec.a (собирается целью libpurec).
# Линкер-скрипт фиксирует базу 0x400000 как в linker-userspace.ld.

ROOT_DIR ?= $(abspath ..)
BIN_DIR ?= $(ROOT_DIR)/bin
PROGRAM_DIR := $(BIN_DIR)/programs
LIB_DIR := $(BIN_DIR)/lib
LIBPUREC := $(LIB_DIR)/libpurec.a
LD_SCRIPT := $(CURDIR)/ld/userspace.ld

CARGO ?= cargo
TARGET := x86_64-unknown-none
PROFILE_FLAG := --release
TARGET_DIR := $(CURDIR)/target
OUT_DIR := $(TARGET_DIR)/$(TARGET)/release

BINS := hello init
STAGED := $(BINS:%=$(OUT_DIR)/%)

# Codegen под USER_CFLAGS (-mno-red-zone, -fno-pic/-static, -mcmodel=small)
# + линковка с libpurec.a. Передаем через env, чтобы не зависеть от того,
# как cargo мержит RUSTFLAGS с .cargo/config.toml.
export RUSTFLAGS := \
	-C no-redzone=yes \
	-C relocation-model=static \
	-C code-model=small \
	-C link-arg=-T$(LD_SCRIPT) \
	-C link-arg=-L$(LIB_DIR) \
	-C link-arg=-lpurec

.PHONY: all libpurec bins install clean
all: install

libpurec:
	$(MAKE) -C $(ROOT_DIR)/src/libc

bins: libpurec $(LD_SCRIPT)
	$(CARGO) build $(PROFILE_FLAG) --target $(TARGET)

install: bins
	@mkdir -p $(PROGRAM_DIR)
	cp $(OUT_DIR)/hello $(PROGRAM_DIR)/hello-rs
	cp $(OUT_DIR)/init $(PROGRAM_DIR)/init-rs
	@echo "staged: $(PROGRAM_DIR)/hello-rs $(PROGRAM_DIR)/init-rs"
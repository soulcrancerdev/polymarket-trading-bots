# Variables
TARGET_X86_64 = x86_64-pc-windows-gnu
TARGET_I686 = i686-pc-windows-gnu
PROJECT_NAME = solana-vntr-pumpswap-copytrader # Change this to your project name
CARGO = cargo

# Target to install prerequisites
.PHONY: install
install:
	sudo apt update
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
	curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
	sudo apt-get install -y nodejs
	sudo npm install -g pm2
	npm install -g npm@11.1.0
	sudo apt install -y mingw-w64
	rustup target add $(TARGET_X86_64)
	rustup target add $(TARGET_I686)

# pm2 to install prerequisites
.PHONY: pm2
pm2:
	pm2 start target/release/solana-vntr-pumpswap-copytrader

# Target to build for x86_64 Windows
.PHONY: build-x86_64
build-x86_64:
	@echo "Checking for mingw-w64 toolchain..."
	@which x86_64-w64-mingw32-gcc > /dev/null 2>&1 || (echo "Installing mingw-w64..." && sudo apt-get update && sudo apt-get install -y mingw-w64)
	@echo "Checking for Windows target..."
	@rustup target list --installed | grep -q $(TARGET_X86_64) || rustup target add $(TARGET_X86_64)
	@echo "Building for Windows x86_64..."
	$(CARGO) build --target=$(TARGET_X86_64) --release

# Target to build for i686 Windows
.PHONY: build-i686
build-i686:
	@echo "Checking for mingw-w64 toolchain..."
	@which i686-w64-mingw32-gcc > /dev/null 2>&1 || (echo "Installing mingw-w64..." && sudo apt-get update && sudo apt-get install -y mingw-w64)
	@echo "Checking for Windows target..."
	@rustup target list --installed | grep -q $(TARGET_I686) || rustup target add $(TARGET_I686)
	@echo "Building for Windows i686..."
	$(CARGO) build --target=$(TARGET_I686) --release

# Target to clean the project
.PHONY: clean
clean:
	$(CARGO) clean

# Start the server
.PHONY: start
start:
	pm2 start 0

# Stop the server
.PHONY: stop
stop:
	pm2 stop 0

# Stop the server
.PHONY: build
build:
	$(CARGO) clean
	$(CARGO) build -r

# Default target - run the bot (validates config, builds, and starts)
.PHONY: default run
default: run

run:
	@echo "========================================"
	@echo "Polymarket Copy Trading Bot"
	@echo "========================================"
	@echo ""
	@if [ ! -f .env ]; then \
		echo "[ERROR] .env file not found!"; \
		echo ""; \
		echo "Please create a .env file first:"; \
		echo "  1. Copy .env.example to .env"; \
		echo "     cp .env.example .env"; \
		echo "  2. Open .env in a text editor"; \
		echo "  3. Fill in your configuration values"; \
		echo "  4. See docs/02_SETUP_GUIDE.md for help"; \
		echo ""; \
		exit 1; \
	fi
	@echo "[1/3] Validating configuration..."
	@echo ""
	@cargo run --release --bin validate_setup || ( \
		echo ""; \
		echo "Configuration check failed! Please fix the errors above."; \
		echo "See docs/06_TROUBLESHOOTING.md for help."; \
		echo ""; \
		exit 1 \
	)
	@echo ""
	@echo "[2/3] Building bot (this may take a few minutes on first run)..."
	@echo ""
	@cargo build --release || ( \
		echo ""; \
		echo "Build failed! Please check the errors above."; \
		echo "See docs/06_TROUBLESHOOTING.md for help."; \
		echo ""; \
		exit 1 \
	)
	@echo ""
	@echo "[3/3] Starting bot..."
	@echo ""
	@echo "Press Ctrl+C to stop the bot"
	@echo ""
	@cargo run --release || ( \
		echo ""; \
		echo "Bot has stopped."; \
		exit 1 \
	)

# Target to display help
.PHONY: help
help:
	@echo "Makefile commands:"
	@echo "  install       - Install necessary packages and configure Rust targets"
	@echo "  build-x86_64  - Build for 64-bit Windows"
	@echo "  build-i686    - Build for 32-bit Windows"
	@echo "  clean         - Clean the target directory"
	@echo "  run           - Run the bot (validates config, builds, and starts) [default]"
	@echo "  help          - Display this help message"
	@echo "  start         - Start the server"
	@echo "  stop          - Stop the server"
	@echo "  build         - Build the server"
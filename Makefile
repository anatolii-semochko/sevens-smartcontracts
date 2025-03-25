# Sevens Smart Contracts - Makefile
# Standard Anchor development commands

.PHONY: install build test deploy clean lint

# ================================
# 📦 SETUP & DEPENDENCIES
# ================================

install:
	yarn install

# ================================
# 🏗️  BUILD
# ================================

build:
	anchor build

# ================================
# 🧪 TESTING
# ================================

test:
	anchor test

test-skip-deploy:
	anchor test --skip-deploy

# ================================
# 🚀 DEPLOYMENT
# ================================

deploy:
	anchor deploy

deploy-local:
	anchor deploy --provider.cluster localnet

# ================================
# 🛠️  UTILITIES
# ================================

clean:
	anchor clean

lint:
	yarn lint

lint-fix:
	yarn lint:fix

check-program-id:
	bash ./check-program-id.sh

validate-idl:
	bash ./validate-idl.sh target/idl/sevens_token.json
	bash ./validate-idl.sh target/idl/sevens_token_management.json

# ================================
# 🌍 SOLANA CLI SHORTCUTS
# ================================

balance:
	solana balance

airdrop:
	solana airdrop 2

programs:
	solana program show --programs

config:
	solana config get
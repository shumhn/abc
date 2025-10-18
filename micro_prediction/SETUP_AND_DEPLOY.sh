#!/bin/bash
# Complete Setup & Deployment Script for Micro-Prediction App
# This installs all dependencies and deploys your Solana program

set -e  # Exit on error

echo "🚀 Micro-Prediction App - Complete Setup & Deployment"
echo "========================================================"
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if running on macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo "⚠️  This script is optimized for macOS"
    echo "For Linux/Windows, adjust install commands accordingly"
fi

echo -e "${BLUE}Step 1: Installing Rust${NC}"
if command -v rustc &> /dev/null; then
    echo "✓ Rust already installed: $(rustc --version)"
else
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
    echo "✓ Rust installed"
fi

echo ""
echo -e "${BLUE}Step 2: Installing Solana CLI${NC}"
if command -v solana &> /dev/null; then
    echo "✓ Solana already installed: $(solana --version)"
else
    echo "Installing Solana CLI..."
    sh -c "$(curl -sSfL https://release.solana.com/stable/install)"
    export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
    echo "✓ Solana CLI installed"
fi

echo ""
echo -e "${BLUE}Step 3: Installing Anchor${NC}"
if command -v anchor &> /dev/null; then
    echo "✓ Anchor already installed: $(anchor --version)"
else
    echo "Installing Anchor Version Manager (avm)..."
    cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
    
    echo "Installing latest Anchor..."
    avm install latest
    avm use latest
    echo "✓ Anchor installed"
fi

echo ""
echo -e "${BLUE}Step 4: Configuring Solana for Devnet${NC}"
solana config set --url devnet
echo "✓ Configured for devnet"

echo ""
echo -e "${BLUE}Step 5: Setting up Keypair${NC}"
if [ -f "$HOME/.config/solana/id.json" ]; then
    echo "✓ Keypair already exists"
    solana address
else
    echo "Creating new keypair..."
    solana-keygen new --outfile ~/.config/solana/id.json --no-bip39-passphrase
    echo "✓ Keypair created"
fi

echo ""
echo -e "${BLUE}Step 6: Airdropping SOL for Devnet${NC}"
echo "Current balance: $(solana balance)"
echo "Requesting airdrop..."
solana airdrop 2 || echo "⚠️  Airdrop might have failed (rate limit). Try again later or use faucet."
echo "New balance: $(solana balance)"

echo ""
echo -e "${BLUE}Step 7: Installing Node Dependencies${NC}"
if [ -f "package.json" ]; then
    if command -v yarn &> /dev/null; then
        yarn install
    else
        npm install
    fi
    echo "✓ Dependencies installed"
fi

echo ""
echo -e "${BLUE}Step 8: Building the Program${NC}"
echo "This may take 2-5 minutes..."
anchor build

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Program built successfully!${NC}"
else
    echo -e "${YELLOW}⚠️  Build failed. Check errors above.${NC}"
    exit 1
fi

echo ""
echo -e "${BLUE}Step 9: Getting Program ID${NC}"
PROGRAM_ID=$(solana address -k target/deploy/micro_prediction-keypair.json)
echo "Your Program ID: ${PROGRAM_ID}"
echo ""
echo -e "${YELLOW}⚠️  IMPORTANT: Update this Program ID in:${NC}"
echo "  1. programs/micro_prediction/src/lib.rs → declare_id!(\"${PROGRAM_ID}\");"
echo "  2. Anchor.toml → micro_prediction = \"${PROGRAM_ID}\""
echo ""
read -p "Have you updated the Program ID? (y/n) " -n 1 -r
echo ""
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo ""
    echo "Please update the files above, then run:"
    echo "  anchor build"
    echo "  anchor deploy"
    exit 0
fi

echo ""
echo -e "${BLUE}Step 10: Rebuilding with Updated Program ID${NC}"
anchor build

echo ""
echo -e "${BLUE}Step 11: Deploying to Devnet${NC}"
anchor deploy --provider.cluster devnet

if [ $? -eq 0 ]; then
    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}🎉 SUCCESS! Program Deployed!${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""
    echo "Program ID: ${PROGRAM_ID}"
    echo ""
    echo "View on Solana Explorer:"
    echo "https://explorer.solana.com/address/${PROGRAM_ID}?cluster=devnet"
    echo ""
    echo -e "${BLUE}Next Steps:${NC}"
    echo "1. Run tests: anchor test --skip-deploy"
    echo "2. Build frontend: cd app && yarn dev"
    echo "3. Read DELEGATION_INTEGRATION.md for MagicBlock setup"
    echo ""
else
    echo -e "${YELLOW}⚠️  Deployment failed. Check balance and errors.${NC}"
    echo "Your balance: $(solana balance)"
    exit 1
fi


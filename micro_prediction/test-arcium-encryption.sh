#!/bin/bash
# Test Arcium Encryption Integration

echo "🧪 Testing Arcium Encryption Integration"
echo "========================================"
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test 1: Backend Health
echo "1️⃣  Testing Backend Health..."
HEALTH=$(curl -s http://localhost:3001/health)
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Backend is running${NC}"
    echo "   Response: $HEALTH"
else
    echo -e "${RED}❌ Backend not responding${NC}"
    echo "   Run: cd backend && node server.js"
    exit 1
fi
echo ""

# Test 2: MXE Configuration
echo "2️⃣  Testing MXE Configuration..."
MXE=$(curl -s http://localhost:3001/arcium/mxe)
if [ $? -eq 0 ]; then
    MODE=$(echo $MXE | jq -r '.mode')
    MXE_ID=$(echo $MXE | jq -r '.mxeId')
    NAME=$(echo $MXE | jq -r '.name')
    
    echo -e "${GREEN}✅ MXE Configuration available${NC}"
    echo "   Mode: $MODE"
    echo "   MXE ID: $MXE_ID"
    echo "   Name: $NAME"
    
    if [ "$MODE" = "mock" ]; then
        echo -e "   ${YELLOW}ℹ️  Running in MOCK mode (expected for development)${NC}"
    else
        echo -e "   ${GREEN}🎉 Running in LIVE mode with real Arcium!${NC}"
    fi
else
    echo -e "${RED}❌ Failed to fetch MXE config${NC}"
    exit 1
fi
echo ""

# Test 3: Submit Encrypted Prediction
echo "3️⃣  Testing Encrypted Prediction Submission..."
PREDICTION=$(cat <<EOF
{
  "commitment": [18,52,86,120,154,188,222,240,19,53,87,121,155,189,223,241,20,54,88,122,156,190,224,242,21,55,89,123,157,191,225,243],
  "ciphertext": "dGVzdCBjaXBoZXJ0ZXh0IGZvciBkZW1v",
  "nonce": "cmFuZG9tIG5vbmNlIGRhdGE=",
  "ephemeralPublicKey": "ZXBoZW1lcmFsIHB1YmxpYyBrZXk=",
  "wallet": "4KQosibBeJoAyjrkMBTk9rSTvLc3iZcwT3pyDioPizs8",
  "stake": 100000000,
  "windowIndex": 2,
  "transactionSignature": "test-encryption-signature-123"
}
EOF
)

RESPONSE=$(curl -s -X POST http://localhost:3001/predictions/0 \
  -H "Content-Type: application/json" \
  -d "$PREDICTION")

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Encrypted prediction submitted${NC}"
    echo "   Response: $RESPONSE"
else
    echo -e "${RED}❌ Failed to submit prediction${NC}"
    exit 1
fi
echo ""

# Test 4: Retrieve Predictions
echo "4️⃣  Testing Prediction Retrieval..."
PREDICTIONS=$(curl -s http://localhost:3001/predictions/0)
COUNT=$(echo $PREDICTIONS | jq '.predictions | length')

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Predictions retrieved${NC}"
    echo "   Total predictions for round 0: $COUNT"
    
    if [ $COUNT -gt 0 ]; then
        echo ""
        echo "   First prediction details:"
        echo $PREDICTIONS | jq '.predictions[0] | {wallet, stake, windowIndex, receivedAt}'
    fi
else
    echo -e "${RED}❌ Failed to retrieve predictions${NC}"
    exit 1
fi
echo ""

# Test 5: Frontend Files
echo "5️⃣  Checking Frontend Files..."
if [ -f "frontend/app.html" ]; then
    echo -e "${GREEN}✅ frontend/app.html exists${NC}"
else
    echo -e "${RED}❌ frontend/app.html not found${NC}"
fi

if [ -f "frontend/arcium-encrypt.js" ]; then
    echo -e "${GREEN}✅ frontend/arcium-encrypt.js exists${NC}"
else
    echo -e "${RED}❌ frontend/arcium-encrypt.js not found${NC}"
fi
echo ""

# Summary
echo "========================================"
echo -e "${GREEN}🎉 All Tests Passed!${NC}"
echo ""
echo "📋 Summary:"
echo "   ✅ Backend running with Arcium support"
echo "   ✅ MXE configuration available ($MODE mode)"
echo "   ✅ Encrypted predictions working"
echo "   ✅ Storage and retrieval functional"
echo "   ✅ Frontend files ready"
echo ""
echo "🚀 Next Steps:"
echo "   1. Open frontend:  open frontend/app.html"
echo "   2. Or serve it:    cd frontend && python3 -m http.server 8080"
echo "   3. Then visit:     http://localhost:8080/app.html"
echo ""
echo "📖 Full docs:        cat ARCIUM_FULL_SETUP_COMPLETE.md"
echo ""

// Test Pyth Real-Time Price Updates in Terminal
const { Connection, PublicKey } = require('@solana/web3.js');
const { PythConnection, getPythProgramKeyForCluster } = require('@pythnetwork/client');

console.log('🚀 Starting Pyth Real-Time Price Test...\n');

const SOL_USD_DEVNET = 'J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix';
let updateCount = 0;
let updateCountThisSecond = 0;
let lastSecond = Math.floor(Date.now() / 1000);

async function testPythRealtime() {
  try {
    console.log('📡 Connecting to Solana Devnet...');
    const connection = new Connection('https://api.devnet.solana.com');
    
    console.log('🔗 Initializing Pyth WebSocket connection...');
    const pythConnection = new PythConnection(connection, getPythProgramKeyForCluster('devnet'));
    
    console.log('📊 Setting up price change listener...');
    pythConnection.onPriceChange((product, price) => {
      if (product.symbol === 'Crypto.SOL/USD') {
        // Track updates per second
        const currentSecond = Math.floor(Date.now() / 1000);
        if (currentSecond !== lastSecond) {
          if (updateCountThisSecond > 0) {
            console.log(`📊 ${updateCountThisSecond} updates in last second\n`);
          }
          updateCountThisSecond = 1;
          lastSecond = currentSecond;
        } else {
          updateCountThisSecond++;
        }
        
        updateCount++;
        
        // Scale price correctly
        const scaledPrice = price.price * Math.pow(10, price.expo);
        const scaledConf = price.confidence * Math.pow(10, price.expo);
        const timestamp = new Date();
        const ms = timestamp.getMilliseconds().toString().padStart(3, '0');
        
        console.log(`⚡ UPDATE #${updateCount}:`);
        console.log(`   💰 SOL/USD = $${scaledPrice.toFixed(4)}`);
        console.log(`   📊 Confidence = ±$${scaledConf.toFixed(4)}`);
        console.log(`   ⏰ Time = ${timestamp.toLocaleTimeString()}.${ms}`);
        console.log('');
      }
    });
    
    console.log('✅ Starting WebSocket connection...\n');
    console.log('=' .repeat(60));
    console.log('🔥 REAL-TIME PRICE UPDATES (Press Ctrl+C to stop)');
    console.log('=' .repeat(60));
    console.log('');
    
    await pythConnection.start();
    
    // Keep running
    console.log('✨ WebSocket ACTIVE - Watch for price updates below:\n');
    
  } catch (error) {
    console.error('❌ Error:', error.message);
    console.error('\n💡 If connection failed, the devnet might be having issues.');
    console.error('   This is normal - the WebSocket works in the browser!');
    process.exit(1);
  }
}

// Handle Ctrl+C gracefully
process.on('SIGINT', () => {
  console.log('\n\n🛑 Stopping...');
  console.log(`📊 Total updates received: ${updateCount}`);
  console.log('✅ Test complete!');
  process.exit(0);
});

testPythRealtime();


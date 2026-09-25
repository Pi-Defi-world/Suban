const { ethers } = require('ethers');

console.log('=== Arc Testnet Validator Keypairs ===\n');

for (let i = 1; i <= 3; i++) {
  const wallet = ethers.Wallet.createRandom();
  console.log(`Validator ${i}:`);
  console.log(`  Address:     ${wallet.address}`);
  console.log(`  Private Key: ${wallet.privateKey}`);
  console.log('');
}

// Also generate a deployer key
const deployer = ethers.Wallet.createRandom();
console.log('Deployer:');
console.log(`  Address:     ${deployer.address}`);
console.log(`  Private Key: ${deployer.privateKey}`);

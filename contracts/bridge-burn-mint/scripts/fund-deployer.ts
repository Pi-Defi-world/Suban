import * as StellarSdk from "@stellar/stellar-sdk";

const HORIZON_URL = "https://api.testnet.minepi.com";
const NETWORK_PASSPHRASE = "Pi Testnet";

// Existing faucet account (never commit secrets; set FAUCET_SECRET in the environment)
function getFaucetSecret(): string {
  const secret = process.env.FAUCET_SECRET;
  if (!secret) {
    throw new Error("FAUCET_SECRET is required");
  }
  return secret;
}

// New deployer wallet to fund
const DEPLOYER_PUBLIC = "GAVRTDGVIRBQTNX5AWZPH2PRIS43HQ5WW2N4CGS426SOBC2EXPTKOHJF";

async function main() {
  const server = new StellarSdk.Horizon.Server(HORIZON_URL);
  const faucetKeypair = StellarSdk.Keypair.fromSecret(getFaucetSecret());

  console.log("Loading faucet account...");
  const faucetAccount = await server.loadAccount(faucetKeypair.publicKey());

  console.log("Building createAccount transaction...");
  const transaction = new StellarSdk.TransactionBuilder(faucetAccount, {
    fee: StellarSdk.BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(
      StellarSdk.Operation.createAccount({
        destination: DEPLOYER_PUBLIC,
        startingBalance: "100", // 100 Pi
      })
    )
    .setTimeout(StellarSdk.TimeoutInfinite)
    .build();

  transaction.sign(faucetKeypair);

  console.log("Submitting transaction...");
  const result = await server.submitTransaction(transaction);
  console.log("Success! Transaction hash:", result.hash);
}

main().catch((error) => {
  console.error("Error:", error);
  process.exit(1);
});

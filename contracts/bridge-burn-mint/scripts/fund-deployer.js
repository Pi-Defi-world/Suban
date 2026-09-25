const StellarSdk = require("@stellar/stellar-sdk");

const HORIZON_URL = "https://api.testnet.minepi.com";
const NETWORK_PASSPHRASE = "Pi Testnet";

const FAUCET_SECRET = "SCB2NN44YEITKM2TEXCTPP3LB33DXFI3M7PKCVJU24UFELY6TTFGOD44";
const DEPLOYER_PUBLIC = "GAVRTDGVIRBQTNX5AWZPH2PRIS43HQ5WW2N4CGS426SOBC2EXPTKOHJF";

async function main() {
  const server = new StellarSdk.Horizon.Server(HORIZON_URL);
  const faucetKeypair = StellarSdk.Keypair.fromSecret(FAUCET_SECRET);

  console.log("Loading faucet account...");
  const faucetAccount = await server.loadAccount(faucetKeypair.publicKey());
  console.log("Faucet sequence:", faucetAccount.sequenceNumber());

  console.log("Building createAccount transaction...");
  const transaction = new StellarSdk.TransactionBuilder(faucetAccount, {
    fee: "1000000",
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(
      StellarSdk.Operation.createAccount({
        destination: DEPLOYER_PUBLIC,
        startingBalance: "100",
      })
    )
    .setTimeout(StellarSdk.TimeoutInfinite)
    .build();

  transaction.sign(faucetKeypair);

  console.log("Submitting transaction...");
  try {
    const result = await server.submitTransaction(transaction);
    console.log("Success! Transaction hash:", result.hash);
  } catch (error) {
    if (error.response && error.response.data) {
      console.error("Transaction failed:");
      console.error(JSON.stringify(error.response.data, null, 2));
    } else {
      console.error("Error:", error.message);
    }
  }
}

main().catch((error) => {
  console.error("Error:", error);
  process.exit(1);
});

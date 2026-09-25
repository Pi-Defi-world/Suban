import { ethers } from "hardhat";

async function main() {
  const [deployer] = await ethers.getSigners();
  console.log("Deploying with:", deployer.address);

  // ── Config ──
  const MAX_SUPPLY = ethers.parseUnits("100000000", 6); // 100M PUSD
  const VOLUME_CAP = ethers.parseUnits("1000000", 6);    // 1M PUSD per window
  const VOLUME_WINDOW = 3600;                              // 1 hour

  // Validator set (separate from Stellar signers)
  // TODO: Replace with actual Arc testnet validator addresses
  const VALIDATORS = [
    "0x0000000000000000000000000000000000000001",
    "0x0000000000000000000000000000000000000002",
    "0x0000000000000000000000000000000000000003",
  ];
  const THRESHOLD = 2;

  // ── Deploy PUSDToken ──
  const PUSD = await ethers.getContractFactory("PUSDToken");
  const pusd = await PUSD.deploy(MAX_SUPPLY, deployer.address);
  await pusd.waitForDeployment();
  const pusdAddr = await pusd.getAddress();
  console.log("PUSDToken deployed:", pusdAddr);

  // ── Deploy ArcBridge ──
  const Bridge = await ethers.getContractFactory("ArcBridge");
  const bridge = await Bridge.deploy(
    pusdAddr,
    VALIDATORS,
    THRESHOLD,
    VOLUME_CAP,
    VOLUME_WINDOW
  );
  await bridge.waitForDeployment();
  const bridgeAddr = await bridge.getAddress();
  console.log("ArcBridge deployed:", bridgeAddr);

  // ── Grant roles to bridge ──
  const MINTER_ROLE = await pusd.MINTER_ROLE();
  const BURNER_ROLE = await pusd.BURNER_ROLE();
  await pusd.grantRole(MINTER_ROLE, bridgeAddr);
  await pusd.grantRole(BURNER_ROLE, bridgeAddr);
  console.log("Granted MINTER + BURNER roles to bridge");

  // ── Set bridge address in PUSDToken ──
  await pusd.setBridge(bridgeAddr);
  console.log("Set bridge address in PUSDToken");

  // ── Summary ──
  console.log("\n── Deployment Summary ──");
  console.log("PUSDToken:", pusdAddr);
  console.log("ArcBridge:", bridgeAddr);
  console.log("Validators:", VALIDATORS);
  console.log("Threshold:", THRESHOLD);
  console.log("Max Supply:", ethers.formatUnits(MAX_SUPPLY, 6), "PUSD");
  console.log("Volume Cap:", ethers.formatUnits(VOLUME_CAP, 6), "PUSD/hour");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

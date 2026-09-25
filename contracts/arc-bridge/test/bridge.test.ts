import { expect } from "chai";
import { ethers } from "hardhat";
import { PUSDToken, ArcBridge } from "../typechain-types";

describe("Arc Bridge", function () {
  let pusd: PUSDToken;
  let bridge: ArcBridge;
  let deployer: any;
  let validator1: any;
  let validator2: any;
  let validator3: any;
  let user: any;

  const MAX_SUPPLY = ethers.parseUnits("100000000", 6);
  const VOLUME_CAP = ethers.parseUnits("1000000", 6);
  const VOLUME_WINDOW = 3600;

  beforeEach(async function () {
    [deployer, validator1, validator2, validator3, user] = await ethers.getSigners();

    const PUSD = await ethers.getContractFactory("PUSDToken");
    pusd = await PUSD.deploy(MAX_SUPPLY, deployer.address);
    await pusd.waitForDeployment();

    const Bridge = await ethers.getContractFactory("ArcBridge");
    bridge = await Bridge.deploy(
      await pusd.getAddress(),
      [validator1.address, validator2.address, validator3.address],
      2,
      VOLUME_CAP,
      VOLUME_WINDOW
    );
    await bridge.waitForDeployment();

    // Grant roles
    const MINTER_ROLE = await pusd.MINTER_ROLE();
    const BURNER_ROLE = await pusd.BURNER_ROLE();
    await pusd.grantRole(MINTER_ROLE, await bridge.getAddress());
    await pusd.grantRole(BURNER_ROLE, await bridge.getAddress());
    await pusd.setBridge(await bridge.getAddress());
  });

  describe("PUSDToken", function () {
    it("should have correct name and symbol", async function () {
      expect(await pusd.name()).to.equal("Pi USD");
      expect(await pusd.symbol()).to.equal("PUSD");
    });

    it("should have correct max supply", async function () {
      expect(await pusd.MAX_SUPPLY()).to.equal(MAX_SUPPLY);
    });

    it("should only allow bridge to mint", async function () {
      await expect(
        pusd.mint(user.address, 1000)
      ).to.be.reverted;
    });

    it("should only allow bridge to burn", async function () {
      await expect(
        pusd.burn(user.address, 1000)
      ).to.be.reverted;
    });
  });

  describe("ArcBridge", function () {
    it("should have correct threshold", async function () {
      expect(await bridge.threshold()).to.equal(2);
    });

    it("should have correct validators", async function () {
      expect(await bridge.isValidator(validator1.address)).to.be.true;
      expect(await bridge.isValidator(validator2.address)).to.be.true;
      expect(await bridge.isValidator(validator3.address)).to.be.true;
    });

    it("should not allow minting with invalid signatures", async function () {
      const amount = ethers.parseUnits("100", 6);
      const sourceChain = "stellar";
      const sourceTxHash = ethers.keccak256(ethers.toUtf8Bytes("tx1"));
      const sourceNonce = 0;

      // Build the bridge hash
      const bridgeHash = ethers.keccak256(
        ethers.solidityPacked(
          ["string", "bytes32", "uint256", "uint256", "address"],
          [sourceChain, sourceTxHash, sourceNonce, amount, user.address]
        )
      );

      // Sign with wrong signer (user, not validator)
      const digest = ethers.keccak256(
        ethers.solidityPacked(
          ["string", "bytes32"],
          ["\x19Ethereum Signed Message:\n32", bridgeHash]
        )
      );
      const sig = await user.signMessage(ethers.getBytes(bridgeHash));

      await expect(
        bridge.mintPusd(
          user.address,
          amount,
          sourceChain,
          sourceTxHash,
          sourceNonce,
          [sig]
        )
      ).to.be.revertedWith("Bridge: insufficient signatures");
    });

    it("should allow burning PUSD", async function () {
      // First, mint some PUSD to user via bridge
      const amount = ethers.parseUnits("1000", 6);
      const sourceChain = "stellar";
      const sourceTxHash = ethers.keccak256(ethers.toUtf8Bytes("tx1"));
      const sourceNonce = 0;

      // Build hash and get valid signatures
      const bridgeHash = ethers.keccak256(
        ethers.solidityPacked(
          ["string", "bytes32", "uint256", "uint256", "address"],
          [sourceChain, sourceTxHash, sourceNonce, amount, user.address]
        )
      );

      const sig1 = await validator1.signMessage(ethers.getBytes(bridgeHash));
      const sig2 = await validator2.signMessage(ethers.getBytes(bridgeHash));

      await bridge.mintPusd(
        user.address,
        amount,
        sourceChain,
        sourceTxHash,
        sourceNonce,
        [sig1, sig2]
      );

      expect(await pusd.balanceOf(user.address)).to.equal(amount);

      // Now burn
      await pusd.connect(user).approve(await bridge.getAddress(), amount);
      await bridge.connect(user).burnPusd("arc", amount);

      expect(await pusd.balanceOf(user.address)).to.equal(0);
    });
  });
});

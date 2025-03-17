import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CheeseRush } from "../target/types/cheese_rush";
import { PublicKey } from "@solana/web3.js";
import { expect } from "chai";

describe("cheeserush-program", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.cheeseRush as Program<CheeseRush>;

  const playerKeypair = provider.wallet.payer;
  const playerPda = PublicKey.findProgramAddressSync(
    [Buffer.from("player"), provider.wallet.publicKey.toBuffer()],
    program.programId
  )[0];

  it("Initialize player", async () => {
    await program.methods
      .initializePlayer(null)
      .accounts({
        owner: playerKeypair.publicKey,
      })
      .rpc();
    const playerAccount = await program.account.player.fetch(playerPda);
    expect(playerAccount.owner.equals(playerKeypair.publicKey)).to.be.true;
    expect(playerAccount.mouseLevel).to.equal(1);
    expect(playerAccount.bros.length).to.equal(1);
  });

  it("Start rush", async () => {
    await program.methods
      .startRush()
      .accounts({
        player: playerPda,
      })
      .rpc();
    const playerAccount = await program.account.player.fetch(playerPda);
    expect(playerAccount.lastRushStart.toNumber()).to.be.greaterThan(0);
    expect(playerAccount.rushDuration).gte(15);
  });

  it("Claim rush fails before completion", async () => {
    try {
      await program.methods
        .claimRush()
        .accounts({
          player: playerPda,
          referrer: null,
        })
        .rpc();
      expect.fail("Expected RushNotComplete error");
    } catch (e) {
      expect(e).to.be.instanceOf(anchor.AnchorError);
      expect((e as anchor.AnchorError).error.errorCode.code).to.equal("RushNotComplete");
    }
  });

  it("Claim rush succeeds after completion", async () => {
    const playerAccount = await program.account.player.fetch(playerPda);
    const rushDuration = playerAccount.rushDuration;
    // Wait until rush should be complete
    await new Promise((resolve) => setTimeout(resolve, (rushDuration + 1) * 1000));

    const playerAccountBefore = await program.account.player.fetch(playerPda);
    await program.methods
      .claimRush()
      .accounts({
        player: playerPda,
        referrer: null,
      })
      .rpc();
    const playerAccountAfter = await program.account.player.fetch(playerPda);
    expect(playerAccountAfter.cheeseBalance.toNumber()).to.be.greaterThan(playerAccountBefore.cheeseBalance.toNumber());
    expect(playerAccountAfter.lastRushStart.toNumber()).to.equal(0);
  });

  it("Check inventory and use boost if available", async () => {
    const playerAccount = await program.account.player.fetch(playerPda);
    if (playerAccount.inventory.cake > 0) {
      await program.methods
        .startRush()
        .accounts({
          player: playerPda,
        })
        .rpc();
      const before = await program.account.player.fetch(playerPda);
      await program.methods
        .useBoost({ cake: {} })
        .accounts({
          player: playerPda,
        })
        .rpc();
      const after = await program.account.player.fetch(playerPda);
      expect(after.inventory.cake).to.equal(before.inventory.cake - 1);
      expect(after.rushDuration).to.equal(before.rushDuration - 300);
    } else if (playerAccount.inventory.milk > 0) {
      const before = await program.account.player.fetch(playerPda);
      await program.methods
        .useBoost({ milk: {} })
        .accounts({
          player: playerPda,
        })
        .rpc();
      const after = await program.account.player.fetch(playerPda);
      expect(after.inventory.milk).to.equal(before.inventory.milk - 1);
      expect(after.milkBoostExpiry.toNumber()).to.be.greaterThan(before.milkBoostExpiry.toNumber());
    } else if (playerAccount.inventory.burger > 0) {
      await program.methods
        .startRush()
        .accounts({
          player: playerPda,
        })
        .rpc();
      const before = await program.account.player.fetch(playerPda);
      await program.methods
        .useBoost({ burger: {} })
        .accounts({
          player: playerPda,
        })
        .rpc();
      const after = await program.account.player.fetch(playerPda);
      expect(after.inventory.burger).to.equal(before.inventory.burger - 1);
      expect(after.lastRushStart.toNumber()).to.be.lessThan(before.lastRushStart.toNumber());
    }
  });

  it("Claim bro reward", async () => {
    const before = await program.account.player.fetch(playerPda);
    await program.methods
      .claimBrosCheese(0)
      .accounts({
        player: playerPda,
        referrer: null,
      })
      .rpc();
    const after = await program.account.player.fetch(playerPda);
    expect(after.cheeseBalance.toNumber()).gte(before.cheeseBalance.toNumber());
    expect(after.bros[0].lastClaim.toNumber()).to.be.greaterThan(before.bros[0].lastClaim.toNumber());
  });

  it("Try to level up mouse with insufficient cheese", async () => {
    const before = await program.account.player.fetch(playerPda);
    try {
      await program.methods
        .levelUpMouse()
        .accounts({
          player: playerPda,
        })
        .rpc();
      const after = await program.account.player.fetch(playerPda);
      expect(after.mouseLevel).to.equal(before.mouseLevel + 1);
      expect(after.cheeseBalance.toNumber()).to.be.lessThan(before.cheeseBalance.toNumber());
    } catch (e) {
      expect(e).to.be.instanceOf(anchor.AnchorError);
      expect((e as anchor.AnchorError).error.errorCode.code).to.equal("InsufficientCheese");
    }
  });

  it("Level up bro fails with insufficient cheese", async () => {
    const playerBefore = await program.account.player.fetch(playerPda);
    try {
      await program.methods
        .levelUpBro(0)
        .accounts({
          player: playerPda,
        })
        .rpc();
      expect.fail("Expected InsufficientCheese error");
    } catch (e) {
      expect(e).to.be.instanceOf(anchor.AnchorError);
      expect((e as anchor.AnchorError).error.errorCode.code).to.equal("InsufficientCheese");
    }
    console.log(playerBefore);
    console.log(`Cheese: ${playerBefore.cheeseBalance.toNumber()}`);
  });
});

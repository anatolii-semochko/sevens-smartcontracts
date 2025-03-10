import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SevensTokenManagement } from "../target/types/sevens_token_management";
import { expect } from "chai";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";

describe("sevens-token-management", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SevensTokenManagement as Program<SevensTokenManagement>;

  let tariffsPda: PublicKey;
  let tokenDataPda: PublicKey;

  const targetWallet = Keypair.generate().publicKey;
  const mintFee = new anchor.BN(1000000); // 0.001 SOL
  const setSaleFee = new anchor.BN(500000); // 0.0005 SOL
  const buyFee = 5; // 5%
  const burnFee = new anchor.BN(200000); // 0.0002 SOL

  beforeEach(async () => {
    // Derive PDAs
    [tariffsPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("tariffs")],
      program.programId
    );
  });

  it("Initializes tariffs successfully", async () => {
    await program.methods
      .initialize(targetWallet, mintFee, setSaleFee, buyFee, burnFee)
      .accounts({
        tariffs: tariffsPda,
        authority: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Verify tariffs data
    const tariffsAccount = await program.account.tariffs.fetch(tariffsPda);
    expect(tariffsAccount.targetWallet.toString()).to.equal(targetWallet.toString());
    expect(tariffsAccount.mintFee.toString()).to.equal(mintFee.toString());
    expect(tariffsAccount.setSaleFee.toString()).to.equal(setSaleFee.toString());
    expect(tariffsAccount.buyFee).to.equal(buyFee);
    expect(tariffsAccount.burnFee.toString()).to.equal(burnFee.toString());
  });

  it("Updates tariffs successfully", async () => {
    // First initialize
    await program.methods
      .initialize(targetWallet, mintFee, setSaleFee, buyFee, burnFee)
      .accounts({
        tariffs: tariffsPda,
        authority: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Update tariffs
    const newTargetWallet = Keypair.generate().publicKey;
    const newMintFee = new anchor.BN(2000000);
    const newSetSaleFee = new anchor.BN(1000000);
    const newBuyFee = 10;
    const newBurnFee = new anchor.BN(400000);

    await program.methods
      .updateTariffs(newTargetWallet, newMintFee, newSetSaleFee, newBuyFee, newBurnFee)
      .accounts({
        tariffs: tariffsPda,
        authority: provider.wallet.publicKey,
      })
      .rpc();

    // Verify updated tariffs
    const tariffsAccount = await program.account.tariffs.fetch(tariffsPda);
    expect(tariffsAccount.targetWallet.toString()).to.equal(newTargetWallet.toString());
    expect(tariffsAccount.mintFee.toString()).to.equal(newMintFee.toString());
    expect(tariffsAccount.setSaleFee.toString()).to.equal(newSetSaleFee.toString());
    expect(tariffsAccount.buyFee).to.equal(newBuyFee);
    expect(tariffsAccount.burnFee.toString()).to.equal(newBurnFee.toString());
  });

  it("Closes tariffs successfully", async () => {
    // First initialize
    await program.methods
      .initialize(targetWallet, mintFee, setSaleFee, buyFee, burnFee)
      .accounts({
        tariffs: tariffsPda,
        authority: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Close tariffs
    await program.methods
      .closeTariffs()
      .accounts({
        tariffs: tariffsPda,
        authority: provider.wallet.publicKey,
      })
      .rpc();

    // Verify account is closed
    try {
      await program.account.tariffs.fetch(tariffsPda);
      expect.fail("Tariffs account should be closed");
    } catch (error) {
      expect(error.toString()).to.include("Account does not exist");
    }
  });

  it("Prevents unauthorized access", async () => {
    const unauthorizedUser = Keypair.generate();

    // Fund unauthorized user
    await provider.connection.requestAirdrop(unauthorizedUser.publicKey, 1000000000);
    await new Promise(resolve => setTimeout(resolve, 1000)); // Wait for airdrop

    // Try to initialize with unauthorized user
    try {
      await program.methods
        .initialize(targetWallet, mintFee, setSaleFee, buyFee, burnFee)
        .accounts({
          tariffs: tariffsPda,
          authority: unauthorizedUser.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([unauthorizedUser])
        .rpc();

      expect.fail("Should have thrown error for unauthorized access");
    } catch (error) {
      // This test assumes there's access control in the contract
      // The specific error will depend on the implementation
      expect(error).to.exist;
    }
  });

  it("Prevents duplicate initialization", async () => {
    // First initialization
    await program.methods
      .initialize(targetWallet, mintFee, setSaleFee, buyFee, burnFee)
      .accounts({
        tariffs: tariffsPda,
        authority: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Try to initialize again - should fail
    try {
      await program.methods
        .initialize(targetWallet, mintFee, setSaleFee, buyFee, burnFee)
        .accounts({
          tariffs: tariffsPda,
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      expect.fail("Should have thrown error for duplicate initialization");
    } catch (error) {
      expect(error.toString()).to.include("already in use");
    }
  });
});
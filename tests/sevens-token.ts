import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SevensToken } from "../target/types/sevens_token";
import { expect } from "chai";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddress,
} from "@solana/spl-token";

describe("sevens-token", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SevensToken as Program<SevensToken>;

  let mintKeypair: Keypair;
  let userTokenAccount: PublicKey;
  let tokenDataPda: PublicKey;
  let hashRegistryPda: PublicKey;
  let saleDataPda: PublicKey;

  const testHash = "a".repeat(64); // 64 character hex string
  const testAuthor = "Test Author";
  const testDescription = "Test Description";
  const testTokenName = "Test Token";

  beforeEach(async () => {
    mintKeypair = Keypair.generate();

    // Derive PDAs
    [tokenDataPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("token_data"), mintKeypair.publicKey.toBuffer()],
      program.programId
    );

    [hashRegistryPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("hash_registry"), Buffer.from(testHash)],
      program.programId
    );

    [saleDataPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("sale_data"), mintKeypair.publicKey.toBuffer()],
      program.programId
    );

    userTokenAccount = await getAssociatedTokenAddress(
      mintKeypair.publicKey,
      provider.wallet.publicKey
    );
  });

  it("Mints a token successfully", async () => {
    await program.methods
      .mintToken(testAuthor, testHash, testDescription, testTokenName, true)
      .accounts({
        mint: mintKeypair.publicKey,
        tokenData: tokenDataPda,
        hashRegistry: hashRegistryPda,
        saleData: saleDataPda,
        userTokenAccount: userTokenAccount,
        authority: provider.wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([mintKeypair])
      .rpc();

    // Verify token data
    const tokenDataAccount = await program.account.tokenData.fetch(tokenDataPda);
    expect(tokenDataAccount.metadata.author).to.equal(testAuthor);
    expect(tokenDataAccount.metadata.hash).to.equal(testHash);
    expect(tokenDataAccount.metadata.description).to.equal(testDescription);
    expect(tokenDataAccount.metadata.tokenName).to.equal(testTokenName);
    expect(tokenDataAccount.metadata.canBeBurned).to.be.true;
  });

  it("Prevents duplicate hash minting", async () => {
    // First mint
    await program.methods
      .mintToken(testAuthor, testHash, testDescription, testTokenName, true)
      .accounts({
        mint: mintKeypair.publicKey,
        tokenData: tokenDataPda,
        hashRegistry: hashRegistryPda,
        saleData: saleDataPda,
        userTokenAccount: userTokenAccount,
        authority: provider.wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([mintKeypair])
      .rpc();

    // Try to mint with same hash - should fail
    const secondMintKeypair = Keypair.generate();
    const [secondTokenDataPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("token_data"), secondMintKeypair.publicKey.toBuffer()],
      program.programId
    );
    const [secondSaleDataPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("sale_data"), secondMintKeypair.publicKey.toBuffer()],
      program.programId
    );
    const secondUserTokenAccount = await getAssociatedTokenAddress(
      secondMintKeypair.publicKey,
      provider.wallet.publicKey
    );

    try {
      await program.methods
        .mintToken(testAuthor, testHash, testDescription, "Second Token", true)
        .accounts({
          mint: secondMintKeypair.publicKey,
          tokenData: secondTokenDataPda,
          hashRegistry: hashRegistryPda, // Same hash registry
          saleData: secondSaleDataPda,
          userTokenAccount: secondUserTokenAccount,
          authority: provider.wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([secondMintKeypair])
        .rpc();

      expect.fail("Should have thrown error for duplicate hash");
    } catch (error) {
      expect(error.toString()).to.include("HashAlreadyExists");
    }
  });

  it("Sets token for sale", async () => {
    // First mint a token
    await program.methods
      .mintToken(testAuthor, testHash, testDescription, testTokenName, true)
      .accounts({
        mint: mintKeypair.publicKey,
        tokenData: tokenDataPda,
        hashRegistry: hashRegistryPda,
        saleData: saleDataPda,
        userTokenAccount: userTokenAccount,
        authority: provider.wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([mintKeypair])
      .rpc();

    const salePrice = new anchor.BN(1000000000); // 1 SOL in lamports

    await program.methods
      .setSale(true, salePrice)
      .accounts({
        mint: mintKeypair.publicKey,
        saleData: saleDataPda,
        tokenData: tokenDataPda,
        userTokenAccount: userTokenAccount,
        authority: provider.wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    // Verify sale data
    const saleDataAccount = await program.account.saleData.fetch(saleDataPda);
    expect(saleDataAccount.onSale).to.be.true;
    expect(saleDataAccount.priceLamports.toString()).to.equal(salePrice.toString());
  });

  it("Burns a burnable token", async () => {
    // First mint a burnable token
    await program.methods
      .mintToken(testAuthor, testHash, testDescription, testTokenName, true)
      .accounts({
        mint: mintKeypair.publicKey,
        tokenData: tokenDataPda,
        hashRegistry: hashRegistryPda,
        saleData: saleDataPda,
        userTokenAccount: userTokenAccount,
        authority: provider.wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([mintKeypair])
      .rpc();

    // Burn the token
    await program.methods
      .burnToken()
      .accounts({
        mint: mintKeypair.publicKey,
        tokenData: tokenDataPda,
        hashRegistry: hashRegistryPda,
        saleData: saleDataPda,
        userTokenAccount: userTokenAccount,
        authority: provider.wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    // Verify accounts are closed
    try {
      await program.account.tokenData.fetch(tokenDataPda);
      expect.fail("Token data account should be closed");
    } catch (error) {
      expect(error.toString()).to.include("Account does not exist");
    }
  });

  it("Prevents burning non-burnable token", async () => {
    // First mint a non-burnable token
    await program.methods
      .mintToken(testAuthor, testHash, testDescription, testTokenName, false)
      .accounts({
        mint: mintKeypair.publicKey,
        tokenData: tokenDataPda,
        hashRegistry: hashRegistryPda,
        saleData: saleDataPda,
        userTokenAccount: userTokenAccount,
        authority: provider.wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([mintKeypair])
      .rpc();

    // Try to burn - should fail
    try {
      await program.methods
        .burnToken()
        .accounts({
          mint: mintKeypair.publicKey,
          tokenData: tokenDataPda,
          hashRegistry: hashRegistryPda,
          saleData: saleDataPda,
          userTokenAccount: userTokenAccount,
          authority: provider.wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();

      expect.fail("Should have thrown error for non-burnable token");
    } catch (error) {
      expect(error.toString()).to.include("BurnNotAllowed");
    }
  });
});
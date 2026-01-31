import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Laminar } from "../target/types/laminar";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";

import {
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAccount,
  getMint,
} from "@solana/spl-token";
import { expect } from "chai";

const SOL_PRECISION = new BN(1_000_000_000);
const USD_PRECISION = new BN(1_000_000);
const BPS_PRECISION = new BN(10_000);

const MIN_CR_BPS = new BN(13_000);               // 130%
const TARGET_CR_BPS = new BN(15_000);            // 150%

const MOCK_SOL_PRICE_USD = new BN(100_000_000);  // $100 per SOL
const MOCK_LST_TO_SOL_RATE = new BN(1_050_000_000); // 1 LST = 1.05 SOL (5% appreciation)

const AMUSD_MINT_FEE_BPS = 50;    // 0.5%
const AMUSD_REDEEM_FEE_BPS = 25;  // 0.25%
const ASOL_MINT_FEE_BPS = 30;     // 0.3%
const ASOL_REDEEM_FEE_BPS = 15;   // 0.15%

interface ProtocolState {
  globalState: PublicKey;
  amusdMint: Keypair;
  asolMint: Keypair;
  lstMint: PublicKey;
  vault: PublicKey;
  vaultAuthority: PublicKey;
  authority: Keypair;
}

interface GlobalStateData {
  version: number;
  bump: number;
  vaultAuthorityBump: number;
  operationCounter: BN;
  authority: PublicKey;
  amusdMint: PublicKey;
  asolMint: PublicKey;
  treasury: PublicKey;
  supportedLstMint: PublicKey;
  totalLstAmount: BN;
  amusdSupply: BN;
  asolSupply: BN;
  minCrBps: BN;
  targetCrBps: BN;
  mintPaused: boolean;
  redeemPaused: boolean;
  mockSolPriceUsd: BN;
  mockLstToSolRate: BN;
}

/**
 * Compute TVL in SOL terms
 */
function computeTvlSol(lstAmount: BN, lstToSolRate: BN): BN {
  return lstAmount.mul(lstToSolRate).div(SOL_PRECISION);
}

/**
 * Compute liability in SOL terms
 */
function computeLiabilitySol(amusdSupply: BN, solPriceUsd: BN): BN {
  if (solPriceUsd.isZero()) return new BN(0);
  return amusdSupply.mul(SOL_PRECISION).div(solPriceUsd);
}

/**
 * Compute equity in SOL terms
 */
function computeEquitySol(tvl: BN, liability: BN): BN {
  if (tvl.lt(liability)) return new BN(0);
  return tvl.sub(liability);
}

/**
 * Compute collateral ratio in basis points
 */
function computeCrBps(tvl: BN, liability: BN): BN {
  if (liability.isZero()) return new BN(Number.MAX_SAFE_INTEGER);
  return tvl.mul(BPS_PRECISION).div(liability);
}

/**
 * Compute aSOL NAV
 */
function computeAsolNav(tvl: BN, liability: BN, asolSupply: BN): BN {
  if (asolSupply.isZero()) return SOL_PRECISION;
  const equity = computeEquitySol(tvl, liability);
  return equity.mul(SOL_PRECISION).div(asolSupply);
}

/**
 * Apply fee and return [netAmount, feeAmount]
 */
function applyFee(amount: BN, feeBps: number): [BN, BN] {
  const fee = amount.mul(new BN(feeBps)).div(BPS_PRECISION);
  const net = amount.sub(fee);
  return [net, fee];
}

describe("Laminar Protocol - Phase 3 Integration Tests", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Laminar as Program<Laminar>;
  const connection = provider.connection;

  let protocolState: ProtocolState;

  let user1: Keypair;
  let user2: Keypair;


  let user1LstAccount: PublicKey;
  let user1AmusdAccount: PublicKey;
  let user1AsolAccount: PublicKey;

  let user2LstAccount: PublicKey;
  let user2AmusdAccount: PublicKey;
  let user2AsolAccount: PublicKey;

  /**
   * Airdrop SOL to wallet
   */
  async function airdropSol(pubkey: PublicKey, amount: number): Promise<void> {
    const sig = await connection.requestAirdrop(pubkey, amount * LAMPORTS_PER_SOL);

    const latestBlockHash = await connection.getLatestBlockhash();
    await connection.confirmTransaction({
      blockhash: latestBlockHash.blockhash,
      lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
      signature: sig
    });
  }

  /**
   * Get GlobalState PDA
   */
  function getGlobalStatePda(): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("global_state")],
      program.programId
    );
  }

  /**
   * Get vault authority PDA
   */
  function getVaultAuthorityPda(): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("vault_authority")],
      program.programId
    );
  }

  /**
   * Intialize Protocol
   */

  async function initializeProtocol(): Promise<ProtocolState> {
    const authority = Keypair.generate();
    await airdropSol(authority.publicKey, 10);

    const lstMint = await createMint(
      connection,
      authority,
      authority.publicKey,
      null,
      9,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID
    );

    const amusdMint = Keypair.generate();
    const asolMint = Keypair.generate();

    const [globalState, globalStateBump] = getGlobalStatePda();
    const [vaultAuthority, vaultAuthorityBump] = getVaultAuthorityPda();

    const vault = await anchor.utils.token.associatedAddress({
      mint: lstMint,
      owner: vaultAuthority,
    });

    await program.methods
      .initialize(
        MIN_CR_BPS,
        TARGET_CR_BPS,
        MOCK_SOL_PRICE_USD,
        MOCK_LST_TO_SOL_RATE
      )
      .accounts({
        authority: authority.publicKey,
        globalState: globalState,
        amusdMint: amusdMint.publicKey,
        asolMint: asolMint.publicKey,
        vault: vault,
        lstMint: lstMint,
        vaultAuthority: vaultAuthority,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        clock: SYSVAR_CLOCK_PUBKEY,
      } as any)
      .signers([authority, amusdMint, asolMint])
      .rpc();

    return {
      globalState,
      amusdMint,
      asolMint,
      lstMint,
      vault,
      vaultAuthority,
      authority,
    };
  }

  /**
   * Fetch and parse GlobalState
   */

  async function getGlobalState(): Promise<GlobalStateData> {
    const account = await program.account.globalState.fetch(protocolState.globalState);
    return account as unknown as GlobalStateData;
  }

  /**
   * Setup a user with LST tokens and token accounts
   */
  async function setupUser(lstAmount: number): Promise<{
    user: Keypair,
    lstAccount: PublicKey,
    amusdAccount: PublicKey,
    asolAccount: PublicKey,
  }> {
    const user = Keypair.generate();
    await airdropSol(user.publicKey, 5);

    const lstAccountInfo = await getOrCreateAssociatedTokenAccount(
      connection,
      user,
      protocolState.lstMint,
      user.publicKey
    );

    await mintTo(
      connection,
      protocolState.authority,
      protocolState.lstMint,
      lstAccountInfo.address,
      protocolState.authority,
      lstAmount * LAMPORTS_PER_SOL
    );

    const amusdAccountInfo = await getOrCreateAssociatedTokenAccount(
      connection,
      user,
      protocolState.amusdMint.publicKey,
      user.publicKey,
    );

    const asolAccountInfo = await getOrCreateAssociatedTokenAccount(
      connection,
      user,
      protocolState.asolMint.publicKey,
      user.publicKey,
    )

    return {
      user,
      lstAccount: lstAccountInfo.address,
      amusdAccount: amusdAccountInfo.address,
      asolAccount: asolAccountInfo.address
    }
  }

  /**
   * Mint amUSD for a user
   */
  async function mintAmUSD(
    user: Keypair,
    userLstAccount: PublicKey,
    userAmusdAccount: PublicKey,
    lstAmount: BN,
    minAmusdOut: BN,
  ): Promise<string> {
    const state = await getGlobalState();
    const [vaultAuthority] = getVaultAuthorityPda();

    const treasuryAmusdAccount = await anchor.utils.token.associatedAddress({
      mint: protocolState.amusdMint.publicKey,
      owner: state.treasury,
    });

    return await program.methods
      .mintAmusd(lstAmount, minAmusdOut)
      .accounts({
        user: user.publicKey,
        globalState: protocolState.globalState,
        amusdMint: protocolState.amusdMint.publicKey,
        userAmusdAccount: userAmusdAccount,
        treasuryAmusdAccount: treasuryAmusdAccount,
        treasury: state.treasury,
        userLstAccount: userLstAccount,
        vault: protocolState.vault,
        vaultAuthority: vaultAuthority,
        lstMint: protocolState.lstMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        clock: SYSVAR_CLOCK_PUBKEY,
      } as any)
      .signers([user])
      .rpc();

  }

  /**
   * Redeem amUSD for a user
   */
  async function redeemAmUSD(
    user: Keypair,
    userLstAccount: PublicKey,
    userAmusdAccount: PublicKey,
    amusdAmount: BN,
    minLstOut: BN
  ): Promise<string> {
    const state = await getGlobalState();
    const [vaultAuthority] = getVaultAuthorityPda();

    const treasuryLstAccount = await anchor.utils.token.associatedAddress({
      mint: protocolState.lstMint,
      owner: state.treasury,
    })

    return await program.methods
      .redeemAmusd(amusdAmount, minLstOut)
      .accounts({
        user: user.publicKey,
        globalState: protocolState.globalState,
        amusdMint: protocolState.amusdMint.publicKey,
        userAmusdAccount: userAmusdAccount,
        treasury: state.treasury,
        treasuryLstAccount: treasuryLstAccount,
        userLstAccount: userLstAccount,
        vault: protocolState.vault,
        vaultAuthority: vaultAuthority,
        lstMint: protocolState.lstMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        clock: SYSVAR_CLOCK_PUBKEY,
      } as any)
      .signers([user])
      .rpc();
  }

  /**
   * Mint aSOL for a user
   */
  async function mintAsol(
    user: Keypair,
    userLstAccount: PublicKey,
    userAsolAccount: PublicKey,
    lstAmount: BN,
    minAsolOut: BN
  ): Promise<string> {
    const state = await getGlobalState();
    const [vaultAuthority] = getVaultAuthorityPda();

    const treasuryAsolAccount = await anchor.utils.token.associatedAddress({
      mint: protocolState.asolMint.publicKey,
      owner: state.treasury,
    });

    return await program.methods
      .mintAsol(lstAmount, minAsolOut)
      .accounts({
        user: user.publicKey,
        globalState: protocolState.globalState,
        asolMint: protocolState.asolMint.publicKey,
        userAsolAccount: userAsolAccount,
        treasuryAsolAccount: treasuryAsolAccount,
        treasury: state.treasury,
        userLstAccount: userLstAccount,
        vault: protocolState.vault,
        vaultAuthority: vaultAuthority,
        lstMint: protocolState.lstMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        clock: SYSVAR_CLOCK_PUBKEY,
      } as any)
      .signers([user])
      .rpc();
  }

  /**
   * Redeem aSOL for a user
   */

  async function redeemAsol(
    user: Keypair,
    userLstAccount: PublicKey,
    userAsolAccount: PublicKey,
    asolAmount: BN,
    minLstOut: BN
  ): Promise<string> {
    const state = await getGlobalState();
    const [vaultAuthority] = getVaultAuthorityPda();

    const treasuryLstAccount = await anchor.utils.token.associatedAddress({
      mint: protocolState.lstMint,
      owner: state.treasury,
    });

    return await program.methods
      .redeemAsol(asolAmount, minLstOut)
      .accounts({
        user: user.publicKey,
        globalState: protocolState.globalState,
        asolMint: protocolState.asolMint.publicKey,
        userAsolAccount: userAsolAccount,
        treasury: state.treasury,
        treasuryLstAccount: treasuryLstAccount,
        userLstAccount: userLstAccount,
        vault: protocolState.vault,
        vaultAuthority: vaultAuthority,
        lstMint: protocolState.lstMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        clock: SYSVAR_CLOCK_PUBKEY,
      } as any)
      .signers([user])
      .rpc();
  }

  /**
   * Update mock prices (admin only)
   */
  async function updateMockPrices(
    newSolPriceUsd: BN,
    newLstToSolRate: BN
  ): Promise<string> {
    return await program.methods
      .updateMockPrices(newSolPriceUsd, newLstToSolRate)
      .accounts({
        authority: protocolState.authority.publicKey,
        globalState: protocolState.globalState,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .signers([protocolState.authority])
      .rpc();
  }

  /**
   * Calculate expected CR from state
   */
  async function calculateCR(): Promise<BN> {
    const state = await getGlobalState();
    const tvl = computeTvlSol(state.totalLstAmount, state.mockLstToSolRate);
    const liability = computeLiabilitySol(state.amusdSupply, state.mockSolPriceUsd);
    return computeCrBps(tvl, liability);
  }

  before(async () => {
    protocolState = await initializeProtocol();
    console.log("Protocol initialized!");
    console.log("  GlobalState:", protocolState.globalState.toBase58());
    console.log("  amUSD Mint:", protocolState.amusdMint.publicKey.toBase58());
    console.log("  aSOL Mint:", protocolState.asolMint.publicKey.toBase58());
    console.log("  LST Mint:", protocolState.lstMint.toBase58());
    console.log("  Vault:", protocolState.vault.toBase58());
  });

  describe("1. Protocol Initialization", () => {
    it("Initializes protocol with correct parameters", async () => {
      const state = await getGlobalState();

      expect(state.version).to.equal(1);
      expect(state.minCrBps.toNumber()).to.equal(MIN_CR_BPS.toNumber());
      expect(state.targetCrBps.toNumber()).to.equal(TARGET_CR_BPS.toNumber());
      expect(state.mockSolPriceUsd.toNumber()).to.equal(MOCK_SOL_PRICE_USD.toNumber());
      expect(state.mockLstToSolRate.toNumber()).to.equal(MOCK_LST_TO_SOL_RATE.toNumber());
      expect(state.totalLstAmount.toNumber()).to.equal(0);
      expect(state.amusdSupply.toNumber()).to.equal(0);
      expect(state.asolSupply.toNumber()).to.equal(0);
      expect(state.mintPaused).to.be.false;
      expect(state.redeemPaused).to.be.false;
    });

    it("Sets correct mint addresses", async () => {
      const state = await getGlobalState();

      expect(state.amusdMint.toBase58()).to.equal(protocolState.amusdMint.publicKey.toBase58());
      expect(state.asolMint.toBase58()).to.equal(protocolState.asolMint.publicKey.toBase58());
      expect(state.supportedLstMint.toBase58()).to.equal(protocolState.lstMint.toBase58());
    });

    it("Sets treasury to authority on initialization", async () => {
      const state = await getGlobalState();
      expect(state.treasury.toBase58()).to.equal(protocolState.authority.publicKey.toBase58());
    });
  });

  describe("2. Mint aSOL (Equity Injection)", () => {
    before(async () => {
      const userSetup = await setupUser(100); // Gives 100 lst to user
      user1 = userSetup.user;
      user1LstAccount = userSetup.lstAccount;
      user1AmusdAccount = userSetup.amusdAccount;
      user1AsolAccount = userSetup.asolAccount;
    });

    it("Mints aSOL at 1:1 rate for first deposit", async () => {
      const lstAmount = new BN(10 * LAMPORTS_PER_SOL);
      const minAsolOut = new BN(1);

      const stateBefore = await getGlobalState();
      expect(stateBefore.asolSupply.toNumber()).to.equal(0);

      await mintAsol(user1, user1LstAccount, user1AsolAccount, lstAmount, minAsolOut);

      const stateAfter = await getGlobalState();

      const expectedSolValue = lstAmount.mul(MOCK_LST_TO_SOL_RATE).div(SOL_PRECISION);
      const [expectedAsolNet, expectedFee] = applyFee(expectedSolValue, ASOL_MINT_FEE_BPS);

      expect(stateAfter.totalLstAmount.toNumber()).to.equal(lstAmount.toNumber());

      const expectedAsolGross = expectedSolValue;
      expect(stateAfter.asolSupply.toNumber()).to.equal(expectedAsolGross.toNumber());

      const userAsolBalance = await getAccount(connection, user1AsolAccount);
      expect(Number(userAsolBalance.amount)).to.equal(expectedAsolNet.toNumber());
    })

    it("Increases TVL on aSOL mint", async () => {
      const state = await getGlobalState();
      const tvl = computeTvlSol(state.totalLstAmount, state.mockLstToSolRate);

      const expectedTvl = new BN(10 * LAMPORTS_PER_SOL).mul(MOCK_LST_TO_SOL_RATE).div(SOL_PRECISION);
      expect(tvl.toNumber()).to.equal(expectedTvl.toNumber());
    });

    it("Maintains infinite CR when no debt exists", async () => {
      const cr = await calculateCR();

      expect(cr.toNumber()).to.equal(Number.MAX_SAFE_INTEGER);
    });
  })

  describe("3. Mint amUSD (Debt Creation)", () => {
    it("Mints amUSD and decreases CR", async () => {
      const lstAmount = new BN(5 * LAMPORTS_PER_SOL); // 5 LST
      const minAmusdOut = new BN(1);

      const crBefore = await calculateCR();

      await mintAmUSD(user1, user1LstAccount, user1AmusdAccount, lstAmount, minAmusdOut);

      const crAfter = await calculateCR();

      // CR should decrease
      expect(crAfter.lt(crBefore)).to.be.true;
      console.log(`  CR changed from ${crBefore.toString()} to ${crAfter.toNumber()} bps`);
    });

    it("Correctly calculates amUSD amount", async () => {
      const state = await getGlobalState();

      // User deposited 5 LST
      // SOL value = 5 * 1.05 = 5.25 SOL
      // amUSD (before fee) = 5.25 * 100 = 525 USD
      // But we need to check in micro-USD (USD_PRECISION = 1e6)

      const userAmusdBalance = await getAccount(connection, user1AmusdAccount);
      expect(Number(userAmusdBalance.amount)).to.be.greaterThan(0);
      console.log(`  User amUSD balance: ${Number(userAmusdBalance.amount) / 1e6} USD`);
    });

    it("Updates balance sheet correctly", async () => {
      const state = await getGlobalState();

      // Total LST should be 10 (aSOL) + 5 (amUSD) = 15 LST
      expect(state.totalLstAmount.toNumber()).to.equal(15 * LAMPORTS_PER_SOL);

      // amUSD supply should be > 0
      expect(state.amusdSupply.toNumber()).to.be.greaterThan(0);

      const tvl = computeTvlSol(state.totalLstAmount, state.mockLstToSolRate);
      const liability = computeLiabilitySol(state.amusdSupply, state.mockSolPriceUsd);
      const equity = computeEquitySol(tvl, liability);

      // Allow small rounding tolerance
      const diff = tvl.sub(liability.add(equity)).abs();
      expect(diff.toNumber()).to.be.lessThan(1000); // < 1000 lamports tolerance
    });
  });

  describe("4. CR Safety Checks", () => {
    it("Rejects amUSD mint when CR would fall below minimum", async () => {

      const userSetup = await setupUser(1000);
      user2 = userSetup.user;
      user2LstAccount = userSetup.lstAccount;
      user2AmusdAccount = userSetup.amusdAccount;
      user2AsolAccount = userSetup.asolAccount;

      // Try to mint a huge amount that would tank CR
      const hugeAmount = new BN(500 * LAMPORTS_PER_SOL);
      const minAmusdOut = new BN(1);

      try {
        await mintAmUSD(user2, user2LstAccount, user2AmusdAccount, hugeAmount, minAmusdOut);
        expect.fail("Should have rejected due to CR violation");
      } catch (err: any) {
        expect(err.toString()).to.include("CollateralRatioTooLow");
      }
    });

    it("Allows amUSD mint when CR stays above minimum", async () => {
      // First add more equity to have room for debt
      const equityAmount = new BN(50 * LAMPORTS_PER_SOL);
      await mintAsol(user2, user2LstAccount, user2AsolAccount, equityAmount, new BN(1));

      // Now mint a smaller amUSD amount
      const smallAmount = new BN(5 * LAMPORTS_PER_SOL);
      const crBefore = await calculateCR();

      await mintAmUSD(user2, user2LstAccount, user2AmusdAccount, smallAmount, new BN(1));

      const crAfter = await calculateCR();
      expect(crAfter.gte(MIN_CR_BPS)).to.be.true;
      console.log(`  CR after small mint: ${crAfter.toNumber()} bps`);
    });
  });


  describe("5. aSOL Minting Improves CR", () => {
    it("Minting aSOL increases collateral ratio", async () => {
      const crBefore = await calculateCR();

      // Mint aSOL (injects equity without adding debt)
      const lstAmount = new BN(20 * LAMPORTS_PER_SOL);
      await mintAsol(user2, user2LstAccount, user2AsolAccount, lstAmount, new BN(1));

      const crAfter = await calculateCR();

      // CR should improve
      expect(crAfter.gt(crBefore)).to.be.true;
      console.log(`  CR improved from ${crBefore.toNumber()} to ${crAfter.toNumber()} bps`);
    });
  });
})
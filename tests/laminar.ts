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
  async function minAmUSD(
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
})
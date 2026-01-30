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

  }

})
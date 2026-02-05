Hylo

# Introduction

Welcome to Hylo’s documentation! The goal of this site is to provide an accessible overview of Hylo’s products with enough technical details about the underlying protocol to satisfy experienced investors and builders.

![Hero Dark](https://mintcdn.com/hylo/cHepwq6pDV-hDIMh/images/hero-dark.png?fit=max&auto=format&n=cHepwq6pDV-hDIMh&q=85&s=24727447a0740b35d39c8d3a79e653dc)

## 

[​

](https://docs.hylo.so/introduction#what-is-hylo)

What is Hylo?

Hylo is a suite of **Decentralized Finance** (DeFi) products on the Solana blockchain, engineered for scalability and independence from traditional financial infrastructure. In a nutshell, Hylo introduces an innovative decentralized stablecoin system consisting of two symbiotic tokens:

[

## hyUSD

A stablecoin backed by Solana liquid staking tokens (LSTs).





](https://docs.hylo.so/protocol-overview/hyUSD-&-xSOL)[

## xSOL

A tokenized asset enabling long term leveraged exposure to SOL.





](https://docs.hylo.so/protocol-overview/hyUSD-&-xSOL)

## 

[​

](https://docs.hylo.so/introduction#guiding-principles)

Guiding Principles

## Solana Native

We build performant products meant to compose, scale, and evolve with Solana DeFi.

## Decentralized

We do not rely on real world assets for collateral, instead using LSTs as “on chain bonds” to harness network yield.

## Permissionless

Our protocol is autonomous and self-contained. Hylo has no fund manager, third party trading dependencies, or trust gaps.

## Secure

Stability is maintained with financially incentivized risk management mechanisms. Slippage-free liquidity is guaranteed for both tokens at all times.

Background

# Centralized stablecoins

## 

[​

](https://docs.hylo.so/background/centralized-stablecoins#foundation-of-the-on-chain-economy)

Foundation of the On-Chain Economy

Stablecoins are the first crypto asset to have achieved true product-market fit globally, dominating trading pairs across centralized and decentralized exchanges. In 2023, stablecoins facilitated over [$12 trillion in on-chain transactions](https://messari.io/assets/stablecoins), accounted for [over 90% of trades](https://www.coingecko.com/research/publications/stablecoins-statistics), and constituted [between 20-40%](https://defillama.com/) of total value locked (TVL) on the largest DeFi protocols.

## 

[​

](https://docs.hylo.so/background/centralized-stablecoins#centralization-considered-harmful)

Centralization Considered Harmful

DeFi democratizes access to markets through innovative and permissionless investment instruments only possible on the blockchain. Stablecoins play a critical role in DeFi as a fixed store of value and liquidity for traders, analogous to fiat currencies like USD in traditional financial markets. However, the dominant stablecoins on the market today carry significant centralization risks which make them misaligned with DeFi’s stated mission.

- **Dependence on central banks.** USDC and USDT are collateralized by treasury bonds whose yields are subject to the agenda of the Federal Reserve.
- **Un-hedgeable custodial risks.** Evolving country-specific regulations place the bank accounts holding collateral at risk of censorship and seizure.
- **No rewards, only risk.** Circle and Tether internalize the yields generated from backing assets while subjecting users to depegging and inflation.

Recent history has demonstrated the risks inherent to centralized stablecoins, with government agencies and corporations intervening in their issuance, redemption, and transmission.

Background

# Decentralized stablecoins

## 

[​

](https://docs.hylo.so/background/decentralized-stablecoins#the-stablecoin-trilemma)

The Stablecoin Trilemma[](https://docs.hylo.so/background/decentralized-stablecoins#the-stablecoin-trilemma)

The Stablecoin Trilemma posits that a stablecoin implementation can only achieve two of three properties: **decentralization**, **price stability**, and **capital efficiency**.Centralized stablecoins like USDC and USDT solve for **capital efficiency** and **price stability**, maintaining a 1:1 backing ratio while mostly avoiding depegging.Decentralized stablecoins have gained prevalence since 2021. These alternative assets are not exempt from the trilemma and face unique challenges due to the hedging strategies used to mitigate volatile cryptocurrency collateral.

![](https://mintcdn.com/hylo/cHepwq6pDV-hDIMh/images/stablecoin_trilemma.png?fit=max&auto=format&n=cHepwq6pDV-hDIMh&q=85&s=2f74bf26015ff7d4d879b72b3e35cb7a)

## 

[​

](https://docs.hylo.so/background/decentralized-stablecoins#decentralized-stablecoin-strategies)

Decentralized Stablecoin Strategies

### 

[​

](https://docs.hylo.so/background/decentralized-stablecoins#collateral-debt-position-cdp)

Collateral Debt Position (CDP)

Popularized by MakerDAO’s DAI, the core mechanic in CDP stablecoins is debt. The user borrows the stablecoin by depositing digital assets in a 1.5:1 or 2:1 ratio. This over-collateralization hedges the volatility of the digital asset for price **stability** while sacrificing **capital efficiency**. As such, CDPs have struggled with scalability as their growth is tightly linked to on-chain borrowing demand.

### 

[​

](https://docs.hylo.so/background/decentralized-stablecoins#algorithmic-stablecoins)

Algorithmic Stablecoins

The most well-known algorithmic stablecoin in recent history was [UST](https://www.coindesk.com/learn/the-fall-of-terra-a-timeline-of-the-meteoric-rise-and-crash-of-ust-and-luna/) by Terra Labs. UST’s peg was not tied to a collateral asset but rather the minting and burning of LUNA, Terra blockchain’s native currency. In May 2022, a financial attack on LUNA triggered a bank run on UST, vaporizing billions in user funds. Since UST’s collapse, algorithmic stablecoins have been avoided due to their inherently vulnerable tokenomics.

### 

[​

](https://docs.hylo.so/background/decentralized-stablecoins#delta-neutral-synthetic-dollars)

Delta-Neutral Synthetic Dollars

Delta-neutral synthetic dollars like Ethena’s USDe are financially engineered with a [cash and carry](https://www.investopedia.com/terms/c/cashandcarry.asp) trading strategy. Collateral assets are accepted 1:1 to mint the synthetic dollar, while a trading system in the issuer’s backend manages short positions against the same asset. In the event of downward price movement, the shorts becomes profitable to mitigate losses incurred by the collateral pool. While capital efficient and stable, synthetic dollars slip with decentralization due to their dependence on centralized exchanges.

Background

# Long term leverage

Crypto traders love leverage, and one only needs to consider the revenue generated on trading platforms like [Jupiter](https://defillama.com/protocol/jupiter-perpetual-exchange#information), [Drift](https://defillama.com/protocol/drift#information), and [Kamino](https://defillama.com/protocol/kamino#information) to be convinced. Despite its “degen” reputation, leveraged trading plays a crucial role in DeFi, aiding in price discovery and accounting for a significant portion of volume on decentralized exchanges.

## 

[​

](https://docs.hylo.so/background/long-term-leverage#leverage-strategies-and-their-drawbacks)

Leverage Strategies and Their Drawbacks

The two dominant leverage strategies in the cryptocurrency market are perpetual futures and spot borrowing. While these methods differ in their mechanics, they share several significant drawbacks.

### 

[​

](https://docs.hylo.so/background/long-term-leverage#mechanics)

**Mechanics**

## Perpetual Futures (perps)

allow traders to open leveraged long or short positions without an expiry date. In AMM-based perps, traders interact with a liquidity pool, compensating liquidity providers with a funding rate for open positions on the wrong side of the oracle price. In order book-based perps, traders’ orders match against each other while funding rates align prices with the spot market. Both variants require collateral, and traders risk liquidation if the market price slips too far from their prediction.

## Spot Leverage

allows a trader to borrow assets against collateral to increase their position size. One strategy may involve borrowing a stablecoin to purchase a volatile asset, hoping to profit from the spread of the asset’s appreciation over a period of time. The trader then sells the volatile asset and uses the proceeds to repay the stablecoin loan with interest.

### 

[​

](https://docs.hylo.so/background/long-term-leverage#drawbacks)

**Drawbacks**

## High Ongoing Costs

- Perps traders pay an average annualized funding rate of [60% on Jupiter perpetual exchange](https://jup.ag/perps) and similar rates on other exchanges.
- In spot leverage, USDC borrowing rates on lending platforms like [Kamino range between 10% to 40%](https://app.kamino.finance/lending/reserve/7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF/D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59). This means that borrows must generate annualized returns greater than the lending rate to profit.

## Liquidation Risk

Both methods expose users to forced position closures (liquidation) in high volatility periods, often leading to significant losses.

## Health Management

Traders must either actively monitor their positions’ health ratios or place correctly tuned limits and stop losses to avoid liquidation. This aspect creates stress and complexity for all but the most experienced traders.

## 

[​

](https://docs.hylo.so/background/long-term-leverage#long-term-leverage)

Long Term Leverage

The drawbacks to perps and spot lending make current leverage options unsuitable for **long term investment strategies.** With [$2B open interest](https://coinalyze.net/solana/open-interest/) on SOL and recent auction of billions of dollars worth of locked [SOL from the FTX estate](https://cointelegraph.com/news/ftx-estate-offloads-last-highly-discounted-solana-tokens), there is clear demand for such trades. However, there is currently no leveraged financial instrument on the market which can offer the benefits of increased exposure without the drawbacks that make current approaches impractical and risky.

Protocol Overview

# hyUSD & xSOL

Learn about Hylo’s two core tokens - hyUSD stablecoin and xSOL leveraged token

Hylo is a decentralized stablecoin protocol native to Solana. Hylo differs from traditional stablecoins in that its backing collateral consists of SOL LSTs, not cash or treasury bills. The main risk of using SOL as collateral is the price volatility inherent to the cryptocurrency market, discussed in depth [decentralized-stablecoins](https://docs.hylo.so/background/decentralized-stablecoins "mention").Hylo achieves a delta-neutral position in its collateral with a radically different and autonomous strategy made possible by a simple equation and two tokens, hyUSD and xSOL.

## 

[​

](https://docs.hylo.so/protocol-overview/hyUSD-&-xSOL#two-tokens-one-pool)

Two Tokens, One Pool

Hylo emits two tokens: **hyUSD** the flagship stablecoin, and **xSOL** a tokenized leveraged long position on SOL.hyUSD and xSOL are backed by a diverse basket of LSTs called the collateral pool. At any point in time, the sum of the two tokens’ market capitalizations is equivalent to the total dollar value locked in the pool. This property can be expressed with the Hylo invariant equation:Collateral TVL=hyUSD Supply×hyUSD Price+xSOL Supply×xSOL PriceCollateral TVL=hyUSD Supply×hyUSD Price+xSOL Supply×xSOL PriceWhat makes Hylo unique is the symbiotic relationship between xSOL and hyUSD. xSOL absorbs SOL price movements, allowing hyUSD to maintain a 1:1 peg with the US dollar in the face of market volatility. Simultaneously, excess value generated by the protocol’s LST reserves benefits xSOL holders with outsized gains.

## 

[​

](https://docs.hylo.so/protocol-overview/hyUSD-&-xSOL#pricing-hyusd-&-xsol)

Pricing hyUSD & xSOL

The price of the hyUSD token is always fixed at $1 USD - it’s a stablecoin!The price of xSOL is calculated from the amount of “variable reserve” in the collateral pool, which is the excess value not reserved to back hyUSD. The Hylo equation can be rearranged to show how the protocol automatically adjusts xSOL’s price:xSOL Price=Collateral TVL−hyUSD SupplyxSOL SupplyxSOL Price=xSOL SupplyCollateral TVL−hyUSD Supply​As the value of the variable reserve grows, implying a price appreciation in SOL, the price of xSOL increases with effective leverage. Likewise if the SOL price dips the value of xSOL decreases, absorbing the volatility witnessed by the entire pool to maintain hyUSD’s peg.

Example: xSOL price adjustment

![Collateral Volatility Absorption Schema](https://mintcdn.com/hylo/cHepwq6pDV-hDIMh/images/collateral_volatility_absorption_schema.png?fit=max&auto=format&n=cHepwq6pDV-hDIMh&q=85&s=548217bb24c8ca84f87a37f00763135b)

## 

[​

](https://docs.hylo.so/protocol-overview/hyUSD-&-xSOL#effective-leverage-on-xsol)

Effective Leverage on xSOL

Effective leverage is a dynamic measure reflecting the xSOL token’s exposure to price movements in the underlying SOL. It is computed as the ratio of the system TVL to the market capitalization of xSOL:Effective Leverage=Collateral TVLxSOL Market CapEffective Leverage=xSOL Market CapCollateral TVL​xSOL’s leverage fluctuates dynamically with activity. It rises when hyUSD is minted or xSOL is redeemed, as these increase the amount of collateral relative to xSOL supply. Conversely, it falls when hyUSD is burned or new xSOL is minted. The effective leverage is inversely related to the proportion of TVL in xSOL: a higher xSOL fraction results in lower leverage, as shown in the graphic below.

Example: xSOL leverage

![Effective Leverage vs Fraction of TVL in xSOL](https://mintcdn.com/hylo/cHepwq6pDV-hDIMh/images/volatility_absorbtion_schem.png?fit=max&auto=format&n=cHepwq6pDV-hDIMh&q=85&s=afa160ab81c4b5cca30e21538e06c434)

Hylo’s stability mechanisms, detailed in [risk-management](https://docs.hylo.so/protocol-overview/risk-management)**,** ensure that the effective leverage on xSOL stays within a target range.

Protocol Overview

# Strategic Advantages

Understanding the key advantages of Hylo’s dual-token system

Hylo’s innovative dual-token system provides comprehensive solutions to the challenges faced by both stablecoins and leverage products in the current DeFi ecosystem.

### 

[​

](https://docs.hylo.so/protocol-overview/strategic-advantage#hyusd)

hyUSD

## Scalability

hyUSD is made capital efficient by the issuance of xSOL. A relatively small amount of xSOL can support a large supply of hyUSD, allowing Hylo to scale beyond the limits of existing decentralized stablecoins.

## Stability

xSOL acts as a volatility shield, absorbing price movements in the underlying collateral pool to maintain the peg on hyUSD.

## Censorship Resistance

Hylo is entirely independent from banks and other custodians, storing collateral assets on chain in transparent and 24/7 auditable vaults.

## Native Yield

Since hyUSD is backed by Solana LSTs, it can sustain and distribute natural yield in any market condition.

## Deep Liquidity

hyUSD can be redeemed for its net asset value at any time without incurring slippage, regardless of the transaction size. Unlike other stablecoins that can face liquidity shortages during high redemption periods, hyUSD doesn’t face this problem since 100% of its backing assets are liquid and accessible at any time.

![comparison table](https://mintcdn.com/hylo/cHepwq6pDV-hDIMh/images/comparison%20table.png?fit=max&auto=format&n=cHepwq6pDV-hDIMh&q=85&s=6b35cf7ef9c93ee8ab7d7174193753a8)

## No Funding Rate

Unlike a perpetual contract, xSOL does not incur an ongoing funding rate. This makes xSOL an ideal long-term hold for investors seeking leveraged exposure.

## No Liquidation Risk

xSOL is held in the user’s wallet and cannot be forcibly liquidated. Investors can hold their position through market turbulence without fear of losing their trade.

## Variable Leverage

xSOL’s leverage adjusts dynamically with market conditions, offering users a passive way to maintain leveraged exposure with reduced risk.

## Passive Management

Users can hold xSOL without the need for constant monitoring or advanced trading strategies of health ratios, reducing stress and complexity.

## No Slippage

xSOL can be minted or redeemed directly through the protocol without incurring any slippage, regardless of the transaction size.

Protocol Overview

# Earning yield with hyUSD

Learn how to earn yield by providing hyUSD to the Stability Pool

hyUSD holders can earn outsized yields while securing the Hylo ecosystem through the Stability Pool. This key earning feature offers users high rewards in exchange for a calculated risk. Technical details about the Stability Pool can be found in [risk-management](https://docs.hylo.so/protocol-overview/risk-management).

## 

[​

](https://docs.hylo.so/protocol-overview/earning-yield-with-hyUSD#introducing-staked-hyusd)

Introducing Staked hyUSD

Staked hyUSD is a tokenized, yield-bearing version of hyUSD that represents a user’s deposit in the Stability Pool. It’s designed to maximize returns from Hylo’s underlying assets (LSTs) through an effortless user experience.When users deposit hyUSD into Hylo’s Stability Pool, they receive Staked hyUSD tokens representing their pro rata share of the pool. These tokens automatically compound yield at the end of each network epoch—no lockups, no additional actions required. Users benefit from:

- Full liquidity
- Composability with other DeFi protocols
- Seamless, built-in yield
- Direct wallet accessibility

## 

[​

](https://docs.hylo.so/protocol-overview/earning-yield-with-hyUSD#yield-generation)

Yield Generation

Every epoch, LST yields generated by Hylo’s collateral are “harvested” as newly minted hyUSD and automatically compounded into the stability pool. This mechanism causes the Staked hyUSD to continuously appreciate in value.

## 

[​

](https://docs.hylo.so/protocol-overview/earning-yield-with-hyUSD#stability-pool-apy)

Stability Pool APY

Yields for the stability pool are generally multiple times higher than the 7-10% base rate generated by LSTs. The minimum rate when the protocol is healthily collateralized is 15% and can go as high as 30%.Staked hyUSD holders collect rewards from the entirety of Hylo’s TVL, because xSOL and unstaked hyUSD holders do not benefit from the yield on their respective collateral by design. As such, the yields generated by the protocol are shared only among those who stake hyUSD, which is a relatively smaller group. The below chart reflects the “effective yield” on hyUSD compared to what percent of its supply is staked.

![Yield Schema](https://mintcdn.com/hylo/cHepwq6pDV-hDIMh/images/yield_schema.png?fit=max&auto=format&n=cHepwq6pDV-hDIMh&q=85&s=a34370c81813a5be5408ed105c8b36c6)

Example: How the Stability Pool Boosts Yields

## 

[​

](https://docs.hylo.so/protocol-overview/earning-yield-with-hyUSD#risk-considerations)

Risk Considerations

While the Stability Pool offers attractive earning potential, it also serves a crucial role in Hylo’s risk management strategy. Under specific and transparently defined market conditions, deposited hyUSD may be converted to xSOL.Participants are encouraged to thoroughly review the [risk-management](https://docs.hylo.so/protocol-overview/risk-management) documentation to gain a comprehensive understanding of the system and its associated risks.

Protocol Overview

# Get Leverage with xSOL

Learn how to gain leveraged exposure to SOL with xSOL—no liquidation risk, no ongoing costs, and protocol-managed rebalancing.

xSOL is Hylo’s leveraged token, designed for users who want amplified exposure to SOL price movements—without the complexities of margin trading or managing a leveraged position. By simply holding xSOL, you benefit from effective leverage on SOL, with the protocol handling all the mechanics for you.

## 

[​

](https://docs.hylo.so/protocol-overview/get-leverage-with-xSOL#no-liquidation-no-ongoing-costs)

No Liquidation, No Ongoing Costs

Unlike traditional leveraged products, xSOL does not expose holders to liquidation risk or ongoing borrowing costs. This is made possible by Hylo’s automatic rebalancing mechanism: when SOL’s price drops, the protocol automatically deleverages your position, and when SOL’s price rises, it re-leverages, ensuring your exposure is always in line with the system’s effective leverage. There are no margin calls or risk of being forcibly closed out of your position.xSOL is designed to be the perfect tool for stress-free leverage. All you need to do is buy xSOL, hold it in your wallet, and sell when your price target is reached. The protocol takes care of all rebalancing and risk management behind the scenes, so you can focus on your investment strategy without worrying about active position management.

## 

[​

](https://docs.hylo.so/protocol-overview/get-leverage-with-xSOL#fees)

Fees

xSOL has minting and redemption fees that dynamically adjust based on the system’s health. When the system is operating normally (collateral ratio above 150%), fees are minimal to encourage participation. During market stress, fees adjust to incentivize actions that improve system health:

- **Minting fees** decrease during stress to encourage new xSOL creation
- **Redemption fees** increase during stress to discourage xSOL burns

This fee structure ensures that xSOL holders can always enter and exit positions, while the protocol maintains stability through market volatility.For detailed information about fee structures and stability mechanisms, see [risk-management](https://docs.hylo.so/protocol-overview/risk-management).

## 

[​

](https://docs.hylo.so/protocol-overview/get-leverage-with-xSOL#risk)

Risk

While xSOL offers a simple and efficient way to gain leveraged exposure to SOL, it is important to understand the risk of volatility decay. Volatility decay refers to the tendency for leveraged tokens to lose value over time in volatile markets, even if SOL’s price returns to its original level. This occurs because xSOL’s value is path-dependent: large price swings, both up and down, can erode value due to the way leverage amplifies gains and losses. xSOL is best suited for users with a strong directional view on SOL and an understanding of the risks associated with holding leveraged tokens over time.

Protocol Overview

# Collateral Pool: A Basket of LSTs

Understanding Hylo’s collateral pool composition and management

Liquid staking tokens (LSTs) are fungible tokens representing staked SOL delegated to one or more validators on the Solana network. LSTs are analogous to an “on-chain bond”, where users lend SOL to validators in order to receive “interest” generated from block rewards and MEV. The yield bearing aspect of LSTs makes them an ideal backing collateral for a stablecoin like hyUSD.

- **Initial Composition**: At launch, the pool will accept a limited list of major cap LSTs available on Solana.
- **Future Expansion**: The pool may diversify over time to include new and high performing LSTs, adapting to market developments and opportunities.
- **True Pricing**: Hylo does not rely on an oracle price for any transaction involving LSTs. Instead the true price of each LST is evaluated based on the exact amount of staked SOL in the SPL staking pool program, thanks to an integration with [Sanctum](https://sanctum.so/). This means that the Net Asset Value (NAV) calculations for both hyUSD and xSOL are based on the exact SOL price per LST, ensuring consistency and transparency (see [True LST Value equations](https://docs.hylo.so/technical-addendum/hylo-equations#id-1-true-lst-value)).

## 

[​

](https://docs.hylo.so/protocol-overview/collateral-pool-a-basket-of-LST#collateral-pool-management)

Collateral Pool Management

Hylo implements fund administration controls which can be utilized by protocol governance to change the makeup and composition of the collateral pool.

- **LST Registry**: The protocol maintains a registry of all LSTs accepted as collateral, and can add new LSTs as the market evolves.
- **Fee Adjustment**: To maintain ideal ratios of each LST in the pool, the protocol employs a dynamic fee system which may discount or increase fees depending on the LST being deposited or withdrawn. This system incentivizes users to help rebalance the pool through normal usage.
  
  Protocol Overview

# Risk Management

Understanding Hylo’s risk management system and stability mechanisms

## 

[​

](https://docs.hylo.so/protocol-overview/risk-management#collateral-ratio)

Collateral Ratio

Hylo implements a multi-tiered risk management approach centered around one key metric: the system collateral ratio (CR). The CR is a measure of system health, indicating the ready availability of the backing assets behind hyUSD.Collateral Ratio=TVL⋅SOL pricehyUSD supply⋅100%Collateral Ratio=hyUSD supplyTVL⋅SOL price​⋅100%​The system requires a CR over 100% to stably back every dollar worth of hyUSD. When CR is over **150%**, the system is considered fully healthy. If the CR falls to 100% the NAV of xSOL becomes zero and hyUSD loses its hedge, exposing its peg to the full volatility of SOL.Hylo’s risk management system employs two stability mechanisms to keep the CR as high as possible: [mint/redeem controls](https://docs.hylo.so/protocol-overview/risk-management#stability-mode-1-fee-controls) and the [stability pool](https://docs.hylo.so/protocol-overview/risk-management#stability-mode-2-stability-pool-drawdown).

## 

[​

](https://docs.hylo.so/protocol-overview/risk-management#stability-modes)

Stability Modes

The protocol may engage two successive stability modes to defend hyUSD’s peg, determined by two CR thresholds. The rationale behind these specific thresholds is detailed in the [Value at Risk Analysis](https://docs.hylo.so/technical-addendum/value-at-risk-analysis).

- **Stability Mode 1:** Activated when CR drops **below 150%**
- **Stability Mode 2:** Activated when CR drops **below 130%**

## 

[​

](https://docs.hylo.so/protocol-overview/risk-management#stability-mode-1-fee-controls)

Stability Mode 1: Fee Controls

When the collateral ratio drops below 150%, the protocol adjusts minting and redemption fees. Fee controls financially incentivize users to perform actions which increase the CR, while disincentivizing actions which decrease it.**Decreasing the supply of hyUSD**

## Minting Fees (Increasing)

hyUSD minting fees increase to discourage new hyUSD creation

## Redemption Fees (Decreasing)

hyUSD redemption fees decrease to encourage hyUSD burns

**Increasing the supply of xSOL**

## Minting Fees (Decreasing)

xSOL minting fees decrease to encourage new xSOL creation

## Redemption Fees (Increasing)

xSOL redemption fees increase to discourage xSOL burns

![fee table](https://mintcdn.com/hylo/cHepwq6pDV-hDIMh/images/fee%20table.jpg?fit=max&auto=format&n=cHepwq6pDV-hDIMh&q=85&s=55ae9535d7230b0198cd95d9e881fe1c)

## 

[​

](https://docs.hylo.so/protocol-overview/risk-management#stability-mode-2-stability-pool-drawdown)

Stability Mode 2: Stability Pool Drawdown

When the collateral ratio crosses the second stability threshold at 130%, signaling severe market volatility, more drastic measures are taken. Fees to control token supplies are more aggressively adjusted and the stability pool is activated.

The stability pool provides **steady upward pressure** on the Collateral Ratio (CR) when it falls below 130%. Staked hyUSD in pool is converted to xSOL to support CR recovery.

### 

[​

](https://docs.hylo.so/protocol-overview/risk-management#stability-pool-intervention)

Stability Pool Intervention

Introduced in [Earning Yield with hyUSD](https://docs.hylo.so/protocol-overview/earning-yield-with-hyUSD), the stability pool provides users the opportunity to earn multiplied LST yields from the reserve as a reward for securing the protocol.When the collateral ratio falls **below 130%**, staked hyUSD in the stability pool is drawn down and converted to xSOL. The double positive effect of **burning hyUSD** and **minting xSOL** quickly recovers the CR a healthy level.Stability pool users acknowledge this potential swap as a risk, in exchange for financial rewards during normal operation. When users wish to withdraw from the pool during market turbulence, a pro rata share of the minted xSOL is returned to them.

Example: Stability Pool usage

**Starting Scenario: Collateral and Pool Breakdown**We start with a total of **100 SOL** that backs both hyUSD and xSOL:

- **80 SOL** is backing the stablecoin hyUSD.
- **20 SOL** is backing the leveraged token xSOL.
- The initial **collateral ratio (CR)** is **125%**, meaning 100 SOL backs **80 SOL worth of hyUSD debt** (100 SOL / 80 SOL = 125%).

**Scenario: Redeeming hyUSD into xSOL (Stability Pool)**Now, let’s assume we redeem **10 SOL worth of hyUSD** into xSOL. This means we reduce the supply of hyUSD while increasing the supply of xSOL. Here’s how the situation changes:

1. **Before Redemption**:
    - **80 SOL** backs hyUSD.
    - **20 SOL** backs xSOL.
    - Collateral ratio = 125% (100 SOL / 80 SOL).
2. **Redemption**:
    - We **redeem 10 SOL worth of hyUSD** into xSOL.
    - After redemption, **70 SOL** will back hyUSD, while **30 SOL** will back xSOL.
3. **After Redemption**:
    - **70 SOL** now backs hyUSD (down from 80 SOL).
    - **30 SOL** backs xSOL (up from 20 SOL).
    - The new **collateral ratio** is:  
        Collateral Ratio = 100 SOL / 70 SOL = **142.5%**

By redeeming **10 SOL** of hyUSD and minting 10 SOL of xSOL, the collateral ratio improves from **125%** to **142.5%**, which is an increase of **17.5%**.**Why This Works**

- **Leverage and Risk Absorption**: xSOL, being a leveraged token, absorbs more volatility. When hyUSD is redeemed into xSOL, the risk is transferred from the hyUSD to the more volatile xSOL. This allows the collateral ratio to improve efficiently.
- **Efficient Collateral Use**: Redeeming just **10 SOL worth of hyUSD** into xSOL increases the collateral ratio by **17.5%**, showing that xSOL can handle more volatility, thus allowing the system to maintain a higher collateral ratio with minimal SOL redemption.

[Collateral Pool: A Basket of LSTs](https://docs.hylo.so/protocol-overview/collateral-pool-a-basket-of-LST)[Protocol Revenue](https://docs.hylo.so/protocol-overview/protocol-revenue)

[x](https://x.com/hylo_so)[telegram](https://t.me/hylo_so)[website](https://hylo.so/)

[Powered by](https://www.mintlify.com/?utm_campaign=poweredBy&utm_medium=referral&utm_source=hylo)

Risk Management - Hylo Documentation


Protocol Overview

# Protocol Revenue

Understanding Hylo’s revenue streams from fees and LST yield

Hylo is designed to generate revenue through two main channels: **mint/redeem fees** and **LST yield produced by the collateral pool**.

![Protocol Revenue Flow](https://mintcdn.com/hylo/cHepwq6pDV-hDIMh/images/Protocol%20Revenue%20Flow.png?fit=max&auto=format&n=cHepwq6pDV-hDIMh&q=85&s=ee88c957b0468044ec749ae2df311a8f)

## 

[​

](https://docs.hylo.so/protocol-overview/protocol-revenue#lst-yield)

LST Yield

The LSTs held in collateral pool are projected to yield a base APY between 8-11%. A large majority of this yield is allocated to those participating in the stability pool, with the remainder serving as a direct source of revenue for Hylo’s treasury.

## 

[​

](https://docs.hylo.so/protocol-overview/protocol-revenue#mint/redeem-fees)

Mint/Redeem Fees

Minting and redemption fees are the protocol’s primary revenue stream. All trading transactions incur a fee on the order of basis points, with fees varying depending on health of the protocol. See [Risk Management](https://docs.hylo.so/protocol-overview/risk-management) for an overview of the fee structure.

As hyUSD gains distribution as a quoted asset for token pricing, the fee revenue stream may become more important than staking yield. Increased usage will create more arbitrage opportunities, leading to higher minting and redeeming volumes.

Liquid Staking Tokens

# hyloSOL

HyloSOL is Hylo’s yield-bearing LST, designed to deliver competitive staking rewards with minimal fees. SOL deposited into **hyloSOL** is staked exclusively to Hylo’s institutional-grade validator. The [Hylo validator](https://solanabeach.io/validator/hy1oJTV2kX9acsqpwk7hbteqXFw9VDbWvbxoamFEufW) is operated in partnership with [Sentinel](https://www.sentinelstake.com/) and [Phase Labs](https://www.phaselabs.io/) to maximize performance, reliability, and uptime.

## 

[​

](https://docs.hylo.so/liquid-staking-tokens/hylosol#key-details)

Key Details

- **APY:** 7-10% (subject to amount of SOL staked and network conditions)
- **XP Rewards:** Standard multiplier (**1x**)
- **Use Case:** Ideal for users prioritizing sustainable yield while still earning XP in Hylo’s point system.
- **Reserve Integration:** HyloSOL will gradually gain a minor allocation in Hylo’s backing reserves, alongside **jitoSOL**.

[Mint hyloSOL now](https://hylo.so/lst).

[  
](https://docs.hylo.so/protocol-overview/protocol-revenue)


Liquid Staking Tokens

# hyloSOL+

HyloSOL+ is an **XP-focused** LST which simply tracks SOL price and redirects staking yields to Hylo’s growth initiatives. Holders give up yield in exchange for boosted rewards.

## 

[​

](https://docs.hylo.so/liquid-staking-tokens/hylosol-plus#key-details)

Key Details

- **APY:** 0% (all staking yield is redirected to hyloSOL)
- **XP Rewards:** Maximum multiplier (**5x**)
- **Use Case:** Designed for users who want to maximize their XP earnings and leaderboard position while keeping SOL exposure.
- **Ecosystem Incentives:** Initially all yield from **hyloSOL+** will be directed into **hyloSOL** to bootstrap early staking APY. At a later time, **hyloSOL+** yield may be allocated to other distribution initiatives for Hylo products across the Solana ecosystem (e.g. incentives for liquidity pools and money markets).

[Mint hyloSOL+ now](https://hylo.so/lst).

[  
](https://docs.hylo.so/liquid-staking-tokens/hylosol)


Technical Addendum

# Hylo Equations

Technical breakdown of the key equations powering Hylo’s protocol

## 

[​

](https://docs.hylo.so/technical-addendum/hylo-equations#true-lst-value)

True LST value

For calculation purposes, both xSOL and hyUSD are priced using the pure SOL price. However, since they are backed by Liquid Staking Tokens (LSTs), we need to accurately capture the price of these LSTs at any time. Using the market price of LSTs could pose significant problems, especially during severe market volatility, when the peg of these LSTs can be lost due to lack of liquidity and also they have a greater susceptibility to manipulation compared to SOL prices. Therefore, we use the **Sanctum SOL value calculator program** to determine the true LST price based on the amount of SOL held in each LST stake pool.The true price of an LST in SOL can be defined by the following equation:True LST Price=Amount of SOL in Stake PoolTotal LST SupplyTrue LST Price=Total LST SupplyAmount of SOL in Stake Pool​

## 

[​

](https://docs.hylo.so/technical-addendum/hylo-equations#sol/usd-oracle)

SOL/USD Oracle

To calculate the value of hyUSD in SOL, ensuring it remains pegged 1:1 with the USD, we need to have the SOL price. For this, we are using the **Pyth EMA SOL/USD** price oracle.

## 

[​

](https://docs.hylo.so/technical-addendum/hylo-equations#net-asset-value-nav-calculation-for-hyusd-and-xsol)

Net Asset Value (NAV) calculation for hyUSD and xSOL

The Net Asset Value (NAV) defines how much both tokens are worth in SOL. Since the SOL price isn’t stable, the NAV of hyUSD needs to be constantly adjusted according to the SOL price to maintain its 1:1 peg to the USD.The **NAV of hyUSD in SOL** can be calculated using the following equation:hyUSD NAVSOL=1SOL PricehyUSD NAVSOL​=SOL Price1​Based on the **hyUSD NAV in SOL**, we can then calculate the **NAV of xSOL in SOL** using this equation:xSOL NAVSOL=Total SOL in Reserve−(hyUSD NAV×hyUSD Supply)xSOL SupplyxSOL NAVSOL​=xSOL SupplyTotal SOL in Reserve−(hyUSD NAV×hyUSD Supply)​

## 

[​

](https://docs.hylo.so/technical-addendum/hylo-equations#collateral-ratio-calculation)

Collateral Ratio calculation

The Collateral Ratio is a metric indicating the health level of hyUSD. It is extremely important to track it accurately to activate stability modes when needed. The Collateral Ratio of hyUSD can be calculated using the following equation:Collateral Ratio=Total SOL In ReservehyUSD NAV In SOL×hyUSD SupplyCollateral Ratio=hyUSD NAV In SOL×hyUSD SupplyTotal SOL In Reserve​

## 

[​

](https://docs.hylo.so/technical-addendum/hylo-equations#stability-pool-apy-calculation)

Stability pool APY calculation

The stability pool APY is variable and greatly depends on the percentage of SOL value being staked, the APY of the reserve, and the defined percentage of Revenue Distribution.The **percentage of staked SOL** can be calculated as follows:%staked SOL=Total SOL in reservehyUSD Staked Amount×hyUSD NAV in SOL%staked SOL​=hyUSD Staked Amount×hyUSD NAV in SOLTotal SOL in reserve​The **Average Reserve Yield** can be calculated as follows:Average Reserve Yield=∑i=1x(SupplyLSTi×PriceLSTi×APYLSTi)Total SOL In ReserveAverage Reserve Yield=Total SOL In Reserve∑i=1x​(SupplyLSTi​​×PriceLSTi​​×APYLSTi​​)​The percentage of Revenue Distribution is dynamically adapted to stay attractive. If the percentage of staked SOL is low, this percentage may be reduced to maximize treasury profit. During periods when it is high, it may be increased to remain competitive compared to other stablecoins yields.With all of this then we can calculate the **stability pool APY** as follows:APY=Average Reserve Yield×%revenue distribution×%staked SOLAPY=Average Reserve Yield×%revenue distribution​×%staked SOL​

## 

[​

](https://docs.hylo.so/technical-addendum/hylo-equations#xsol-effective-leverage)

xSOL Effective Leverage

The xSOL effective leverage will change constantly. To calculate it, we first need to determine the **virtual xSOL market cap**. This can be calculated as follows:Market CapxSOL=NAVxSOL×SupplyxSOLMarket CapxSOL​=NAVxSOL​×SupplyxSOL​Next, we calculate the **xSOL effective leverage** with this equation:Effective LeveragexSOL=Total SOL In ReserveMarket CapxSOLEffective LeveragexSOL​=Market CapxSOL​Total SOL In Reserve​The effective leverage will tend to exponentially increase as the collateral ratio gets closer to 100%, and will approach 1 (indicating it follows the SOL price perfectly) as the collateral ratio increases.

Technical Addendum

# Value-at-Risk Analysis

Understanding how Hylo uses VaR analysis to set risk management thresholds

Hylo employs [**Value at Risk (VaR)**](https://www.investopedia.com/terms/v/var.asp) analysis to set appropriate thresholds for its risk management metrics. VaR is a statistical technique used to measure and quantify the level of financial risk within the system over a specific time frame.Our model utilizes comprehensive SOL price data spanning from April 10, 2020, to August 15, 2024. This extensive dataset allows for a robust analysis of potential price movements.

## 

[​

](https://docs.hylo.so/technical-addendum/value-at-risk-analysis#var-for-system-cr-and-threshold)

VaR for System CR and Threshold

### 

[​

](https://docs.hylo.so/technical-addendum/value-at-risk-analysis#parameters)

Parameters

- **Data range:** April 10, 2020, to August 15, 2024
- **Confidence level:** 99.9% (representing a 0.1% probability event)
- **Time frame:** 1 day

### 

[​

](https://docs.hylo.so/technical-addendum/value-at-risk-analysis#results-and-threshold-setting)

Results and Threshold Setting

The analysis reveals a 99.9% VaR of -32.95% for a one-day price drop in SOL. Based on this, Hylo has set the minimum System CR threshold at 150%. This threshold ensures that the system can withstand a price drop corresponding to the 0.1% worst day in SOL’s recent history without taking any action.

### 

[​

](https://docs.hylo.so/technical-addendum/value-at-risk-analysis#justification-for-150%-threshold)

Justification for 150% Threshold

This threshold aims to:

1. Safeguard the system against rare but severe price drops, where the probability of a destabilizing event rises above 0.1%
2. Provide ample time for the protocol to act before reaching dangerously low collateralization levels
3. Maintain a conservative stance given SOL’s observed market behavior

## 

[​

](https://docs.hylo.so/technical-addendum/value-at-risk-analysis#var-for-adjusted-cr)

VaR for Adjusted CR

For the Adjusted CR, Hylo uses a longer-term risk assessment based on a 31 days period. The adjusted CR is a more conservative metric that takes into account the stability pool and the available hyUSD liquidity on the market for buyback.

### 

[​

](https://docs.hylo.so/technical-addendum/value-at-risk-analysis#parameters-2)

Parameters

- **Data range:** April 10, 2020, to August 15, 2024
- **Confidence level:** 99.9% (representing a 0.1% probability event)
- **Time frame:** 31 days

### 

[​

](https://docs.hylo.so/technical-addendum/value-at-risk-analysis#results)

Results

This analysis shows a 99.9% VaR of -56.82% for a 31-day price drop in SOL. Based on this calculation, Hylo considers the Adjusted Collateral Ratio healthy if it remains above 230%.This higher threshold ensures that Hylo can absorb a 1-month price drawdown corresponding to the 0.1% worst month of SOL by activating all of its mechanisms without requiring direct action from users. The main action taken to address this metric is through incentives, primarily stability pool rewards. If we see this metric declining too much, we may increase the rewards distributed to stability pool LP to make it more attractive.The use of different time frames (1-day for System CR, 31-day for Adjusted CR) allows Hylo to manage both short-term volatility and longer-term market trends effectively.

## 

[​

](https://docs.hylo.so/technical-addendum/value-at-risk-analysis#var-trend-analysis)

VaR Trend Analysis

Our analysis shows a trend of decreasing VaR for a 1-day period for SOL year over year:

- Full dataset (2020-04-10 to 2024-08-15): 99.9% VaR of -32.95%
- Last 2 years: 99.9% VaR of -28.18%
- Last year: 99.9% VaR of -27.55%

Despite this trend suggesting a maturing market with potentially lower risk, we maintain the 150% threshold for the stability mode activation based on the full historical dataset for comprehensive risk coverage, prudent risk management, and user confidence.Regular reviews of both the System CR and Adjusted CR thresholds will be conducted, with potential future adjustments carefully evaluated to balance robust risk management with capital efficiency.

[  
](https://docs.hylo.so/technical-addendum/hylo-equations)

Technical Addendum

# Additional Risk Management

Understanding Hylo’s additional risk management mechanisms and metrics

## 

[​

](https://docs.hylo.so/technical-addendum/additional-risk-management#minting-and-redeeming-bounty)

Minting and Redeeming Bounty

In most situations where the collateral ratio falls below 130%, the steady and continuous upward pressure from the Rebalancing Pool will quickly restore it, maintaining system stability. Issues only occur if the downward pressure on the CR is sustained or severe enough to exhaust the stability pool’s supply of hyUSD.To speed up and support the CR’s recovery while reducing reliance on rebalancing pool funds, a special reserve of accumulated treasury revenue is available to provide **bonuses** to users who **mint xSOL or redeem hyUSD** in Stability Mode. These bonuses offer users (or bots) an immediate arbitrage opportunity to buy and redeem hyUSD from external liquidity pools.Moreover, the combination of a **minting bonus and high xSOL leverage** under these conditions makes xSOL minting highly appealing. Minting xSOL has an even more significant positive impact on system stability than hyUSD redemptions, and the combination of both provides a robust backstop in any market condition.The combination of the Stability Pool and these bounty mechanisms creates a strong defense against severe price drops, offering multiple layers of protection to ensure the protocol’s stability.

### 

[​

](https://docs.hylo.so/technical-addendum/additional-risk-management#recapitalization)

Recapitalization

In the most extreme scenarios, where insufficient hyUSD is available to stabilize the protocol and the probability of destabilization becomes unacceptably high, Hylo has the ability to implement a recapitalization process. This process utilizes the accumulated treasury funds to recapitalize the protocol.

- Buying and Redeeming hyUSD: Purchasing hyUSD from the market and redeeming it, reducing the supply of hyUSD and improving the CR.

This recapitalization mechanism serves as a last resort to ensure the protocol’s stability in extreme market conditions, providing an additional layer of security for users and stakeholders.

### 

[​

](https://docs.hylo.so/technical-addendum/additional-risk-management#additional-metrics)

Additional Metrics

In addition to the CR, Hylo utilizes two additional metrics to provide a more comprehensive view of the system’s health:

- **Stability Pool Adjusted Collateral Ratio**: This represents the collateral ratio if the entire stability pool’s hyUSD were converted into xSOL. It provides insight into the system’s resilience considering its built-in stability mechanism.
- **The Buyback Adjusted Collateral Ratio at 1% Premium**: This metric shows the collateral ratio if all hyUSD available on decentralized exchanges (DEX) at a maximum 1% premium ($1.01 per hyUSD) were bought back and redeemed for SOL. It offers a view of the system’s health considering immediate market liquidity.

The Adjusted Collateral Ratio, which considers both the stability pool and available liquidity for buyback, is a key metric in Hylo’s risk management strategy. It provides a more conservative and comprehensive view of the system’s health compared to the System CR alone.


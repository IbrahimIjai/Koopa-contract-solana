# Koopaa - Ajo (Adashe) Smart Contract

A blockchain implementation of traditional Ajo/Adashe savings groups on Solana.

## Overview

Koopaa is a decentralized implementation of the traditional Ajo (also known as Adashe, Esusu, or Susu) rotating savings groups common in many cultures. This smart contract enables users to:

1. Create savings groups
2. Join existing groups
3. Make regular contributions
4. Take turns receiving the pooled funds
5. Track participation and distributions

Built on Solana using Anchor and Rust, Koopaa makes traditional savings groups more accessible, transparent, and secure.

## Features

- **Group Creation**: Create groups with customizable parameters such as contribution amount and rotation period
- **Verified Membership**: Join groups with transparent participant tracking
- **Automated Distributions**: Receive funds based on predetermined order
- **Secure Payments**: All transactions secured on Solana's blockchain
- **Fair Protocol Fee**: Small percentage fee to maintain the protocol

## Technical Stack

- **Blockchain**: Solana
- **Smart Contract Framework**: Anchor
- **Programming Language**: Rust
- **Token Standard**: SPL Token

## Getting Started

### Prerequisites

- Solana CLI
- Anchor Framework
- Rust
- Node.js and npm/yarn

### Installation

1. Clone the repository

```bash
git clone https://github.com/yourusername/koopa-contract.git
cd koopa-contract
```

2. Install dependencies

```bash
yarn install
```

3. Build the program

```bash
anchor build
```

4. Test the program

```bash
anchor test
```

## How It Works

1. **Create Group**: A group creator initializes a new Ajo group with parameters
2. **Join Group**: Participants join until the group reaches its target size
3. **Start Group**: The creator starts the group when all slots are filled
4. **Contribute**: Each period, participants contribute the agreed amount
5. **Claim**: The designated recipient for that period claims the pooled funds
6. **Rotate**: The process repeats until all members have received funds

## License

[MIT](LICENSE)

## Acknowledgments

- Traditional Ajo/Adashe savings groups for the inspiration
- Solana ecosystem for providing the infrastructure

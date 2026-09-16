const fs = require('fs');
const path = require('path');
const {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL,
} = require('@solana/web3.js');

async function main() {
  const destAddress = process.argv[2];
  if (!destAddress) {
    console.error('Usage: node send-all.js <destination-address>');
    process.exit(1);
  }

  const secretKey = Uint8Array.from(
    JSON.parse(fs.readFileSync(path.join(__dirname, 'wallet.json'), 'utf8'))
  );
  const from = Keypair.fromSecretKey(secretKey);
  const to = new PublicKey(destAddress);

  const connection = new Connection('https://api.devnet.solana.com', 'confirmed');

  const balance = await connection.getBalance(from.publicKey);
  console.log('From:', from.publicKey.toBase58());
  console.log('Current balance (lamports):', balance);

  if (balance === 0) {
    console.log('Nothing to send.');
    return;
  }

  // Build a transfer tx first with placeholder amount to estimate fee
  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash();

  const tx = new Transaction({
    feePayer: from.publicKey,
    blockhash,
    lastValidBlockHeight,
  }).add(
    SystemProgram.transfer({
      fromPubkey: from.publicKey,
      toPubkey: to,
      lamports: 1, // placeholder, fixed below
    })
  );

  const fee = await tx.getEstimatedFee(connection);
  console.log('Estimated fee (lamports):', fee);

  const sendAmount = balance - fee;
  if (sendAmount <= 0) {
    console.log('Balance too low to cover fee.');
    return;
  }

  tx.instructions[0] = SystemProgram.transfer({
    fromPubkey: from.publicKey,
    toPubkey: to,
    lamports: sendAmount,
  });

  const sig = await sendAndConfirmTransaction(connection, tx, [from]);
  console.log('Sent (lamports):', sendAmount, '=', sendAmount / LAMPORTS_PER_SOL, 'SOL');
  console.log('Signature:', sig);
  console.log('Explorer:', `https://explorer.solana.com/tx/${sig}?cluster=devnet`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

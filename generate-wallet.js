const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

// Base58 (Bitcoin/Solana alphabet) encoder - no external deps needed
const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
function base58encode(bytes) {
  let digits = [0];
  for (let i = 0; i < bytes.length; i++) {
    let carry = bytes[i];
    for (let j = 0; j < digits.length; j++) {
      carry += digits[j] << 8;
      digits[j] = carry % 58;
      carry = (carry / 58) | 0;
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = (carry / 58) | 0;
    }
  }
  // leading zero bytes -> leading '1's
  for (let i = 0; i < bytes.length && bytes[i] === 0; i++) {
    digits.push(0);
  }
  return digits.reverse().map(d => ALPHABET[d]).join('');
}

// Generate an Ed25519 keypair
const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');

// Extract raw 32-byte seed and 32-byte public key from DER/JWK
const jwk = privateKey.export({ format: 'jwk' });
const seed = Buffer.from(jwk.d, 'base64url');   // 32-byte private seed
const pub = Buffer.from(jwk.x, 'base64url');    // 32-byte public key

// Solana CLI keypair format: 64-byte array = seed (32) + pubkey (32)
const secretKey64 = Buffer.concat([seed, pub]);
const secretKeyArray = Array.from(secretKey64);

const address = base58encode(pub);

const outPath = path.join(__dirname, 'wallet.json');
fs.writeFileSync(outPath, JSON.stringify(secretKeyArray));

console.log('Public Key (Address):', address);
console.log('Keypair file written to:', outPath);

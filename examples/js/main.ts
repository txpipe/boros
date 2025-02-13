import { Bip32PrivateKey, mnemonicToEntropy, wordlist } from "@blaze-cardano/core";
import { Blaze, Blockfrost, Core, HotWallet } from "@blaze-cardano/sdk";
import dotenv from 'dotenv';

// Load environment variables
dotenv.config();

const provider = new Blockfrost({
    network:"cardano-preview",
    projectId: process.env.BLOCKFROST_PROJECT_ID as string
});

const mnemonic =
  "end link visit estate sock hurt crucial forum eagle earn idle laptop wheat rookie when hard suffer duty kingdom clerk glide mechanic debris jar";
const entropy = mnemonicToEntropy(mnemonic, wordlist);
const masterkey = Bip32PrivateKey.fromBip39Entropy(Buffer.from(entropy), "");
const wallet = await HotWallet.fromMasterkey(masterkey.hex(), provider);

// Step #4
// Create a Blaze instance from the wallet and provider
const blaze = await Blaze.from(provider, wallet);

// Optional: Print the wallet address
console.log("Wallet address", wallet.address.toBech32());

// Optional: Print the wallet balance
console.log("Wallet balance", (await wallet.getBalance()).toCore());

// Step #5
// Create a example transaction that sends 5 ADA to an address
const tx = await blaze
  .newTransaction()
  .payLovelace(
    Core.Address.fromBech32(
      "addr_test1qzhwefhsv6xn2s4sn8a92f9m29lwj67aykn4plr9xal4r48del5pz2hf795j5wxzhzf405g377jmw7a92k9z2enhd6pqutz67m",
    ),
    5_000_000n,
  )
  .complete();

// Step #6
// Sign the transaction
const signexTx = await blaze.signTransaction(tx);

const base64 = Buffer.from(signexTx.toCbor(), "hex").toString("base64");

// log the grpcurl command string with the base64 string
console.log('Execute this command:\n',`\tgrpcurl -plaintext -d '{"tx": [{"raw": "${base64}"}]}' localhost:50052 utxorpc.v1alpha.submit.SubmitService.SubmitTx`);

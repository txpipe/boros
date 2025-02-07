# Run boros binary

For Boros configuration, one option is to create a config file named `boros.toml` in the root where the binary is executing, or set the env `BOROS_CONFIG` with the path of the config file.

File example [config](config.toml)

## Submit Tx

With Boros running, it's possible to submit a request to boros using the command line below, but change the `YOUR_TX_HEX` for your hex tx.

> [!NOTE]
> Example command tested using Linux(Fedora 41)

Command
```sh
grpcurl -plaintext -d "{\"tx\": [{\"raw\": \"$(echo YOUR_TX_HEX | xxd -r -p | base64 | tr -d '\n')\"}]}" localhost:50052 utxorpc.v1alpha.submit.SubmitService.SubmitTx
```

Your command should be something like this
```sh
grpcurl -plaintext -d "{\"tx\": [{\"raw\": \"$(echo 84a300d9010281825820cdc219e7abe938a35ca074d4bd02d6ccc3c2fc25d1462af07b6c1e8f40933af200018282581d603f79e7eab3ab95c1f78824872ac6fd65f79d120868057f2bd19306f81a3b9aca0082581d603f79e7eab3ab95c1f78824872ac6fd65f79d120868057f2bd19306f81a77c0bd2f021a0002990da100d90102818258205d4b008e92a42846add4d060e49d7427700ced0ab8eb73e559acc14d228ca5475840f3f12cbfd551e5e51f9eb32fcf695c3a63ec3dfb7329108f45b441cafc7a706659d06238665327779e32415c91b6190e0cd00096aee41f6e405be59d69462708f5f6 | xxd -r -p | base64 | tr -d '\n')\"}]}" localhost:50052 utxorpc.v1alpha.submit.SubmitService.SubmitTx
```

## Submit Tx using tx-gen script

This script is only for tests, it creates a wallet and an address, so it generates a transaction and requests boros to send the tx.

> [!NOTE]
> the script requires cardano-cli and a cardano node socket file.

Before executing [tx-gen](tx-gen), set the env `CARDANO_NODE_SOCKET_PATH`.

```sh
export CARDANO_NODE_SOCKET_PATH=""
```

Executing

```
./tx-gen
```

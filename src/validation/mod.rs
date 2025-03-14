use std::{borrow::Cow, sync::Arc};

use pallas::{
    crypto::hash::Hash,
    ledger::{
        primitives::TransactionInput,
        traverse::{wellknown::GenesisValues, MultiEraInput, MultiEraOutput, MultiEraTx},
        validate::{
            phase_one, phase_two,
            uplc::{script_context::SlotConfig, EvalReport},
            utils::{AccountState, CertState, Environment, UTxOs},
        },
    },
};

use crate::ledger::u5c::U5cDataAdapter;

pub async fn validate_tx(
    tx: &MultiEraTx<'_>,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
) -> Result<(), anyhow::Error> {
    let (block_slot, block_hash_vec) = u5c_adapter.fetch_tip().await?;
    let block_hash_vec: [u8; 32] = block_hash_vec.try_into().unwrap();
    let block_hash: Hash<32> = Hash::from(block_hash_vec);

    let tip = (block_slot, block_hash);

    let network_magic = 2;

    let era = tx.era();

    let pparams = u5c_adapter.fetch_pparams(era).await?;

    let genesis_values = GenesisValues::from_magic(network_magic.into()).unwrap();

    let env = Environment {
        prot_params: pparams.clone(),
        prot_magic: network_magic,
        block_slot: tip.0,
        network_id: genesis_values.network_id as u8,
        acnt: Some(AccountState::default()),
    };

    let input_refs = tx
        .requires()
        .iter()
        .map(|input: &MultiEraInput<'_>| (*input.hash(), input.index() as u32))
        .collect::<Vec<(Hash<32>, u32)>>();

    let utxos = u5c_adapter.fetch_utxos(input_refs, era).await?;

    let mut pallas_utxos = UTxOs::new();

    for ((tx_hash, index), eracbor) in utxos.iter() {
        let tx_in = TransactionInput {
            transaction_id: *tx_hash,
            index: (*index).into(),
        };
        let input = MultiEraInput::AlonzoCompatible(Box::from(Cow::Owned(tx_in)));
        let output = MultiEraOutput::try_from(eracbor)?;
        pallas_utxos.insert(input, output);
    }

    phase_one::validate_tx(tx, 0, &env, &pallas_utxos, &mut CertState::default())?;

    Ok(())
}

pub async fn evaluate_tx(
    tx: &MultiEraTx<'_>,
    u5c_adapter: Arc<dyn U5cDataAdapter>,
) -> Result<EvalReport, anyhow::Error> {
    let era = tx.era();

    let pparams = u5c_adapter.fetch_pparams(era).await?;

    let slot_config = SlotConfig::default();

    let input_refs = tx
        .requires()
        .iter()
        .map(|input: &MultiEraInput<'_>| (*input.hash(), input.index() as u32))
        .collect::<Vec<(Hash<32>, u32)>>();

    let utxos = u5c_adapter.fetch_utxos(input_refs, era).await?;

    let utxos = utxos
        .iter()
        .map(|((tx_hash, index), eracbor)| (From::from((*tx_hash, *index)), eracbor.clone()))
        .collect();

    let report = phase_two::evaluate_tx(tx, &pparams, &utxos, &slot_config)
        .map_err(|e| anyhow::anyhow!("Error evaluating transaction: {:?}", e))?;

    Ok(report)
}

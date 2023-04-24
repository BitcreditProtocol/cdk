use std::str::FromStr;

use bitcoin::Amount;

use crate::{
    cashu_mint::CashuMint,
    dhke::construct_proof,
    error::Error,
    types::{
        BlindedMessages, MintKeys, Proof, ProofsStatus, RequestMintResponse, SendProofs,
        SplitPayload, SplitRequest, TokenData,
    },
};

pub struct CashuWallet {
    pub mint: CashuMint,
    pub keys: MintKeys,
}

impl CashuWallet {
    pub fn new(mint: CashuMint, keys: MintKeys) -> Self {
        Self { mint, keys }
    }

    /// Check if a proof is spent
    pub async fn check_proofs_spent(&self, proofs: Vec<Proof>) -> Result<ProofsStatus, Error> {
        let spendable = self.mint.check_spendable(&proofs).await?;

        // Seperate proofs in spent and unspent based on mint response
        let (spendable, spent): (Vec<_>, Vec<_>) = proofs
            .iter()
            .zip(spendable.spendable.iter())
            .partition(|(_, &b)| b);

        Ok(ProofsStatus {
            spendable: spendable.into_iter().map(|(s, _)| s).cloned().collect(),
            spent: spent.into_iter().map(|(s, _)| s).cloned().collect(),
        })
    }

    /// Request Mint
    pub async fn request_mint(&self, amount: Amount) -> Result<RequestMintResponse, Error> {
        self.mint.request_mint(amount).await
    }

    /// Check fee
    pub async fn check_fee(&self, invoice: lightning_invoice::Invoice) -> Result<Amount, Error> {
        Ok(self.mint.check_fees(invoice).await?.fee)
    }

    /// Receive
    pub async fn receive(&self, encoded_token: &str) -> Result<Vec<Proof>, Error> {
        let token_data = TokenData::from_str(encoded_token)?;

        let mut proofs = vec![];
        for token in token_data.token {
            if token.proofs.is_empty() {
                continue;
            }

            let keys = if token.mint.eq(&self.mint.url) {
                self.keys.clone()
            } else {
                // TODO:
                println!("No match");
                self.keys.clone()
                // CashuMint::new(token.mint).get_keys().await.unwrap()
            };

            // Sum amount of all proofs
            let amount = token
                .proofs
                .iter()
                .fold(Amount::ZERO, |acc, p| acc + p.amount);

            let split_payload = self
                .create_split(Amount::ZERO, amount, token.proofs)
                .await?;

            let split_response = self.mint.split(split_payload.split_payload).await?;

            // Proof to keep
            let keep_proofs = construct_proof(
                split_response.fst,
                split_payload.keep_blinded_messages.rs,
                split_payload.keep_blinded_messages.secrets,
                &keys,
            )?;

            // Proofs to send
            let send_proofs = construct_proof(
                split_response.snd,
                split_payload.send_blinded_messages.rs,
                split_payload.send_blinded_messages.secrets,
                &keys,
            )?;

            proofs.push(keep_proofs);
            proofs.push(send_proofs);
        }

        Ok(proofs.iter().flatten().cloned().collect())
    }

    /// Create Split Payload
    pub async fn create_split(
        &self,
        keep_amount: Amount,
        send_amount: Amount,
        proofs: Vec<Proof>,
    ) -> Result<SplitPayload, Error> {
        let keep_blinded_messages = BlindedMessages::random(keep_amount)?;
        let send_blinded_messages = BlindedMessages::random(send_amount)?;

        let outputs = {
            let mut outputs = keep_blinded_messages.blinded_messages.clone();
            outputs.extend(send_blinded_messages.blinded_messages.clone());
            outputs
        };
        let split_payload = SplitRequest {
            amount: send_amount,
            proofs,
            outputs,
        };

        Ok(SplitPayload {
            keep_blinded_messages,
            send_blinded_messages,
            split_payload,
        })
    }

    /// Send
    pub async fn send(&self, amount: Amount, proofs: Vec<Proof>) -> Result<SendProofs, Error> {
        let mut amount_avaliable = Amount::ZERO;
        let mut send_proofs = SendProofs::default();

        for proof in proofs {
            amount_avaliable += proof.amount;
            if amount_avaliable > amount {
                send_proofs.change_proofs.push(proof);
                break;
            } else {
                send_proofs.send_proofs.push(proof);
            }
        }

        if amount_avaliable.lt(&amount) {
            return Err(Error::InsufficantFunds);
        }

        // If amount avaliable is EQUAL to send amount no need to split
        if amount_avaliable.eq(&amount) {
            return Ok(send_proofs);
        }

        let amount_to_keep = amount_avaliable - amount;
        let amount_to_send = amount;

        let split_payload = self
            .create_split(amount_to_keep, amount_to_send, send_proofs.send_proofs)
            .await?;

        let split_response = self.mint.split(split_payload.split_payload).await?;

        // Proof to keep
        let keep_proofs = construct_proof(
            split_response.fst,
            split_payload.keep_blinded_messages.rs,
            split_payload.keep_blinded_messages.secrets,
            &self.keys,
        )?;

        // Proofs to send
        let send_proofs = construct_proof(
            split_response.snd,
            split_payload.send_blinded_messages.rs,
            split_payload.send_blinded_messages.secrets,
            &self.keys,
        )?;

        Ok(SendProofs {
            change_proofs: keep_proofs,
            send_proofs,
        })
    }
}

use std::{ops::Deref, sync::Arc};

use cashu::nuts::nut00::wallet::BlindedMessages as BlindedMessagesSdk;

use crate::{error::Result, Amount, BlindedMessage, SecretKey};

pub struct BlindedMessages {
    inner: BlindedMessagesSdk,
}

impl Deref for BlindedMessages {
    type Target = BlindedMessagesSdk;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl BlindedMessages {
    pub fn random(amount: Arc<Amount>) -> Result<Self> {
        Ok(Self {
            inner: BlindedMessagesSdk::random(*amount.as_ref().deref())?,
        })
    }

    pub fn blank(fee_reserve: Arc<Amount>) -> Result<Self> {
        Ok(Self {
            inner: BlindedMessagesSdk::blank(*fee_reserve.as_ref().deref())?,
        })
    }

    pub fn blinded_messages(&self) -> Vec<Arc<BlindedMessage>> {
        self.inner
            .blinded_messages
            .clone()
            .into_iter()
            .map(|b| Arc::new(b.into()))
            .collect()
    }

    pub fn secrets(&self) -> Vec<String> {
        self.inner.secrets.clone()
    }

    pub fn rs(&self) -> Vec<Arc<SecretKey>> {
        self.inner
            .rs
            .clone()
            .into_iter()
            .map(|s| Arc::new(s.into()))
            .collect()
    }

    pub fn amounts(&self) -> Vec<Arc<Amount>> {
        self.inner
            .amounts
            .clone()
            .into_iter()
            .map(|a| Arc::new(a.into()))
            .collect()
    }
}

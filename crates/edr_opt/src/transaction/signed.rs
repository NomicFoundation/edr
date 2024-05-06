use edr_eth::transaction::{
    Eip1559SignedTransaction, Eip155SignedTransaction, Eip2930SignedTransaction,
    LegacySignedTransaction,
};

use super::deposited;

pub enum Transaction {
    PreEip155Legacy(LegacySignedTransaction),
    PostEip155Legacy(Eip155SignedTransaction),
    Eip2930(Eip2930SignedTransaction),
    Eip1559(Eip1559SignedTransaction),
    Deposited(deposited::Transaction),
}

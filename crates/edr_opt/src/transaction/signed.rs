pub enum Transaction {
    PreEip155Legacy(LegacySignedTransaction),
    PostEip155Legacy(Eip155SignedTransaction),
    Eip2930(Eip2930SignedTransaction),
    Eip1559(Eip1559SignedTransaction),
    Deposited(deposited::Transaction),
}

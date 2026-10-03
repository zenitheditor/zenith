mod common;
use common::*;
use zenith_tx::op::{OpPathAnchor, OpPathSubpath};
use zenith_tx::{Op, OpSpan, Permissions, Position, Transaction, TxStatus, run_transaction};

#[path = "add_remove/addnode.rs"]
mod addnode;
#[path = "add_remove/duplicatenode.rs"]
mod duplicatenode;
#[path = "add_remove/duplicatepage.rs"]
mod duplicatepage;
#[path = "add_remove/opacity.rs"]
mod opacity;
#[path = "add_remove/removenode.rs"]
mod removenode;
#[path = "add_remove/replacetext.rs"]
mod replacetext;

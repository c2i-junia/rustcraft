//! The tables of the guide `controls`.

use super::Table;
use crate::codex::markup::Block;

pub fn table(t: &Table) -> Result<Vec<Block>, String> {
    t.unknown()
}

// Currently using 32 - each hash byte goes into its own cell, this might be
// compressed for optimization purposes in the future.
pub const HASH_WIDTH: usize = 32; // number of columns used for hash output
pub const KECCAK_INPUT_WIDTH: usize = 1;
pub const KECCAK_OUTPUT_WIDTH: usize = 1;
// for S: RLP_NUM cells + HASH_WIDTH cells
// for C: RLP_NUM cells + HASH_WIDTH cells
pub const RLP_NUM: usize = 2; // how many bytes are RLP specific before hash output starts in branch children rows
pub const S_RLP_START: usize = 0;
pub const S_START: usize = RLP_NUM; // at which position starts hash of a children in S proof
pub const C_RLP_START: usize = RLP_NUM + HASH_WIDTH; // at which position C RLP bytes start
pub const C_START: usize = RLP_NUM + HASH_WIDTH + RLP_NUM; // at which position starts hash of a children in C proof
pub const WITNESS_ROW_WIDTH: usize = 2 * HASH_WIDTH + 2 * RLP_NUM; // number of columns used where the main data appear (other columns are selectors)

// how many rows the branch occupy: 1 (branch init) + 16 (branch children) + 2 (extension node S and C):
pub const BRANCH_ROWS_NUM: i32 = 19;
pub const EXTENSION_ROWS_NUM: i32 = 2; // how many extension rows

pub const R_TABLE_LEN: usize = 32; // how many elements has an array of powers of randomness r

// branch init row:
// the first 4 bytes are used for specifying how many RLP specific bytes this branch has
pub const BRANCH_0_S_START: usize = 4; // at which position branch RLP bytes start for S proof
pub const BRANCH_0_C_START: usize = 7; // at which position branch RLP bytes start for C proof
pub const BRANCH_0_KEY_POS: usize = 10; // which branch node is being modified
pub const IS_BRANCH_S_PLACEHOLDER_POS: usize = 11; // is S branch just a placeholder
pub const IS_BRANCH_C_PLACEHOLDER_POS: usize = 12; // is C branch just a placeholder
pub const DRIFTED_POS: usize = 13; // to which position in a newly added branch the leaf drifted
// when generating key or address RLC whether modified_node of a branch needs to be multiplied
// by 16 (if there are nibbles n0 n1 ... n63, rlc = (n0 * 16 + n1) + (n2 * 16 + n3) * r + ... (n62 * 16 + n63) * r^31)
pub const IS_BRANCH_C16_POS: usize = 19;
// when generating key or address RLC whether modified_node of a branch needs to be multiplied
// by 1
pub const IS_BRANCH_C1_POS: usize = 20;
// whether it is an extension node with 1 byte (short) and its modified_node needs to be multiplied by 16:
pub const IS_EXT_SHORT_C16_POS: usize = 21;
// whether it is an extension node with 1 byte (short) and its modified_node needs to be multiplied by 1:
pub const IS_EXT_SHORT_C1_POS: usize = 22;
// whether it is an extension node with more than one byte (long), the number of bytes is even,
// and its modified_node needs to be multiplied by 16:
pub const IS_EXT_LONG_EVEN_C16_POS: usize = 23;
// whether it is an extension node with more than one byte (long), the number of bytes is even,
// and its modified_node needs to be multiplied by 1:
pub const IS_EXT_LONG_EVEN_C1_POS: usize = 24;
// whether it is an extension node with more than one byte (long), the number of bytes is odd,
// and its modified_node needs to be multiplied by 16:
pub const IS_EXT_LONG_ODD_C16_POS: usize = 25;
// whether it is an extension node with more than one byte (long), the number of bytes is odd,
// and its modified_node needs to be multiplied by 1:
pub const IS_EXT_LONG_ODD_C1_POS: usize = 26;
// Note that C16/C1 in extension node refer to the multiplier to be used with branch modified_node,
// not with the extension node first nibble.

// while short/long above means having one or more than one nibbles, the constants below mean whether
// the whole extension node (not only nibbles) has more than 55 bytes
pub const IS_S_EXT_LONGER_THAN_55_POS: usize = 27;
pub const IS_C_EXT_LONGER_THAN_55_POS: usize = 28;

// whether branch (in S proof) in the extension node is hashed or not (means whether branch is longer than 31 bytes)
pub const IS_S_BRANCH_IN_EXT_HASHED_POS: usize = 29;
// whether branch (in C proof) in the extension node is hashed or not
pub const IS_C_BRANCH_IN_EXT_HASHED_POS: usize = 30;

// First level means the rows of the first node in a proof (it can be branch or account leaf).
// Note that if there are multiple proofs chained (the previous C root corresponds to the current S root),
// the first level appear at the beginning of each of the chained proofs.

// row.len() - NOT_FIRST_LEVEL_POS holds the information whether the node is in the first level:
pub const NOT_FIRST_LEVEL_POS: usize = 2;
// row.len() - IS_STORAGE_MOD_POS holds the information whether the proof is about storage modification
pub const IS_STORAGE_MOD_POS: usize = 3;
// row.len() - IS_NONCE_MOD_POS holds the information whether the proof is about nonce modification
pub const IS_NONCE_MOD_POS: usize = 4;
// row.len() - IS_BALANCE_MOD_POS holds the information whether the proof is about balance modification
pub const IS_BALANCE_MOD_POS: usize = 5;
pub const IS_CODEHASH_MOD_POS: usize = 6; // TODO: to be removed
// row.len() - IS_ACCOUNT_DELETE_MOD_POS holds the information whether the proof is about account delete modification
pub const IS_ACCOUNT_DELETE_MOD_POS: usize = 7;
// row.len() - IS_NON_EXISTING_ACCOUNT_POS holds the information whether the proof shows the account does not exist 
pub const IS_NON_EXISTING_ACCOUNT_POS: usize = 8;
pub const COUNTER_WITNESS_LEN: usize = 4; // TODO: probably to be removed (state circuit will possess intermediate roots instead)
pub const COUNTER_POS: usize = IS_NON_EXISTING_ACCOUNT_POS + COUNTER_WITNESS_LEN;

// indexes for storage leaf:
pub const LEAF_KEY_S_IND: i32 = 0;
pub const LEAF_VALUE_S_IND: i32 = 1;
pub const LEAF_KEY_C_IND: i32 = 2;
pub const LEAF_VALUE_C_IND: i32 = 3;
pub const LEAF_DRIFTED_IND: i32 = 4;

// indexes for account leaf:
pub const ACCOUNT_LEAF_KEY_S_IND: i32 = 0;
pub const ACCOUNT_LEAF_KEY_C_IND: i32 = 1;
pub const ACCOUNT_NON_EXISTING_IND: i32 = 2;
pub const ACCOUNT_LEAF_NONCE_BALANCE_S_IND: i32 = 3;
pub const ACCOUNT_LEAF_NONCE_BALANCE_C_IND: i32 = 4;
pub const ACCOUNT_LEAF_STORAGE_CODEHASH_S_IND: i32 = 5;
pub const ACCOUNT_LEAF_STORAGE_CODEHASH_C_IND: i32 = 6;
pub const ACCOUNT_DRIFTED_LEAF_IND: i32 = 7;
pub const ACCOUNT_LEAF_ROWS: i32 = 8;

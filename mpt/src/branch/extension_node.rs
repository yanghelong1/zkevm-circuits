use halo2_proofs::{
    circuit::Region,
    plonk::{Advice, Column, ConstraintSystem, Expression, Fixed, VirtualCells},
    poly::Rotation,
};
use itertools::Itertools;
use pairing::arithmetic::FieldExt;
use std::marker::PhantomData;

use crate::{
    columns::{AccumulatorCols, MainCols, PositionCols},
    helpers::{
        bytes_expr_into_rlc, compute_rlc, get_bool_constraint, get_is_extension_node,
        get_is_extension_node_even_nibbles, get_is_extension_node_long_odd_nibbles,
        get_is_extension_node_one_nibble,
    },
    mpt::{MPTConfig, ProofValues},
    param::{
        ACCOUNT_LEAF_ROWS, ACCOUNT_LEAF_STORAGE_CODEHASH_C_IND,
        ACCOUNT_LEAF_STORAGE_CODEHASH_S_IND, BRANCH_ROWS_NUM, C_RLP_START, C_START, HASH_WIDTH,
        IS_BRANCH_C16_POS, IS_BRANCH_C1_POS, IS_BRANCH_C_PLACEHOLDER_POS,
        IS_BRANCH_S_PLACEHOLDER_POS, IS_C_EXT_LONGER_THAN_55_POS, IS_C_EXT_NODE_NON_HASHED_POS,
        IS_EXT_LONG_EVEN_C16_POS, IS_EXT_LONG_EVEN_C1_POS, IS_EXT_LONG_ODD_C16_POS,
        IS_EXT_LONG_ODD_C1_POS, IS_EXT_SHORT_C16_POS, IS_EXT_SHORT_C1_POS,
        IS_S_EXT_LONGER_THAN_55_POS, IS_S_EXT_NODE_NON_HASHED_POS, KECCAK_INPUT_WIDTH,
        KECCAK_OUTPUT_WIDTH, NIBBLES_COUNTER_POS, RLP_NUM,
    },
    witness_row::MptWitnessRow,
};

use super::BranchCols;

/*
A branch occupies 19 rows:
BRANCH.IS_INIT
BRANCH.IS_CHILD 0
...
BRANCH.IS_CHILD 15
BRANCH.IS_EXTENSION_NODE_S
BRANCH.IS_EXTENSION_NODE_C

Example:

[1 0 1 0 248 81 0 248 81 0 14 1 0 6 1 0 0 0 0 1 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 160 29 143 36 49 6 106 55 88 195 10 34 208 147 134 155 181 100 142 66 21 255 171 228 168 85 11 239 170 233 241 171 242 0 160 29 143 36 49 6 106 55 88 195 10 34 208 147 134 155 181 100 142 66 21 255 171 228 168 85 11 239 170 233 241 171 242 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[0 160 135 117 154 48 1 221 143 224 133 179 90 254 130 41 47 5 101 84 204 111 220 62 215 253 155 107 212 69 138 221 91 174 0 160 135 117 154 48 1 221 143 224 133 179 90 254 130 41 47 5 101 84 204 111 220 62 215 253 155 107 212 69 138 221 91 174 1]
[0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1]
[226 30 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 160 30 252 7 160 150 158 68 221 229 48 73 181 91 223 120 156 43 93 5 199 95 184 42 20 87 178 65 243 228 156 123 174 0 16]
[0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 160 30 252 7 160 150 158 68 221 229 48 73 181 91 223 120 156 43 93 5 199 95 184 42 20 87 178 65 243 228 156 123 174 0 17]

The last two rows present the extension node. This might be a bit misleading, because the extension
node appears in the trie above the branch (the first 17 rows).

The constraints in `extension_node.rs` check:
 - RLP of the node
 - that the branch hash is in the extension node (the bytes `[30 252 ... 174]` in extension node rows present the hash
of the underlying branch)
 - that the hash of the extension is in the parent node.
*/

#[derive(Clone, Debug)]
pub(crate) struct ExtensionNodeConfig<F> {
    _marker: PhantomData<F>,
}

/*
Let's say we have branch1 and branch2 below it.

branch1 S row 0 || branch1 C row 0
...
branch1 S row 15 || branch1 C row 15

branch2 S row 0 || branch2 C row 0
...
branch2 S row 15 || branch2 C row 15

Hash of branch2 S is in one of the branch1 rows (in S columns).
Hash of branch2 C is in one of the branch1 rows (in C columns).

In what follows, we write branch without S and C - it is the same for both cases.

Key key1 determines the position of branch2 hash in branch1 (0-15).
To check this, branch2 RLC (over all RLP bytes - all 1+16 rows, 1 is for branch init row)
is checked to have a hash in branch1, at modified_node index
(modified_node depends on key key1).

However, with extension node it's a bit different.

branch1 S row 0 || branch1 C row 0
...
branch1 S row 15 || branch1 C row 15
extension1 S
extension1 C

branch2 S row 0 || branch2 C row 0
...
branch2 S row 15 || branch2 C row 15
extension2 S
extension2 C

There are additional rows immediately after branch 16 rows - one row for
extension node S and one row for extension node C. These rows are empty when
we have a regular branch.

Let's say branch2 is extension node. In this case, extension2 row contains:
  - key bytes that present the extension
  - hash of branch2

---
Example 1:

Key extension of length 2:
[228, 130, 0,          149,        160, 114,                    253,                     150,133,18,192,156,19,241,162,51,210,24,1,151,16,48,7,177,42,60,49,34,230,254,242,79,132,165,90,75,249]
[rlp, rlp, key byte 1, key byte 2, rlp, hash of branch2 byte 0, hash of branch2 byte 1, ...]
[0, 149] presents key extension:
  - 0 because it's even length (in branch it would be 16, see terminator)
  - 149 = 9*16 + 5
Key extension is [9, 5].

Two constraints are needed:
  - the hash of extension node (extension node RLC is needed) needs to be
    checked whether it's in branch1
  - the hash of branch2 is in extension node.

Also, it needs to be checked that key extension corresponds to key1 (not in this chip).

---
Example 2:

Key extension of length 1:
[226, 16,        160,172,105,12...
[rlp, key byte1, ...
[16] presents key extension:
  - 16 = 0 + 16
Key extension is [0].

*/

impl<F: FieldExt> ExtensionNodeConfig<F> {
    #[allow(clippy::too_many_arguments)]
    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        q_enable: impl Fn(&mut VirtualCells<'_, F>) -> Expression<F>,
        inter_root: Column<Advice>,
        position_cols: PositionCols<F>,
        is_account_leaf_in_added_branch: Column<Advice>,
        branch: BranchCols<F>,
        s_main: MainCols<F>,
        c_main: MainCols<F>,
        accs: AccumulatorCols<F>,
        keccak_table: [Column<Fixed>; KECCAK_INPUT_WIDTH + KECCAK_OUTPUT_WIDTH],
        r_table: Vec<Expression<F>>,
        is_s: bool,
        acc_r: F,
    ) -> Self {
        let config = ExtensionNodeConfig {
            _marker: PhantomData,
        };
        let one = Expression::Constant(F::from(1_u64));
        let c33 = Expression::Constant(F::from(33));
        let c128 = Expression::Constant(F::from(128));
        let c160_inv = Expression::Constant(F::from(160_u64).invert().unwrap());
        let c192 = Expression::Constant(F::from(192));
        let c248 = Expression::Constant(F::from(248));
        let mut rot_into_branch_init = -17;
        if !is_s {
            rot_into_branch_init = -18;
        }

        meta.create_gate("Extension node RLC", |meta| {
            let mut constraints = vec![];
            let q_not_first = meta.query_fixed(position_cols.q_not_first, Rotation::cur());
            let q_enable = q_enable(meta);
            let is_branch_init_prev = meta.query_advice(branch.is_init, Rotation::prev());

            let mut rot = 0;
            if !is_s {
                rot = -1;
            }

            /*
            s_rlp1, s_rlp2, s_bytes need to be the same in both extension rows.
            However, to make space for nibble witnesses, we put nibbles in
            extension row C s_bytes. So we use s_bytes from S row.
            */

            let s_rlp1 = meta.query_advice(s_main.rlp1, Rotation(rot));
            let mut rlc = s_rlp1;
            let s_rlp2 = meta.query_advice(s_main.rlp2, Rotation(rot));
            rlc = rlc + s_rlp2 * r_table[0].clone();

            let s_bytes_rlc = compute_rlc(
                meta,
                s_main.bytes.to_vec(),
                1,
                one.clone(),
                rot,
                r_table.clone(),
            );
            rlc = rlc + s_bytes_rlc;

            let acc_s = meta.query_advice(accs.acc_s.rlc, Rotation(rot));

            /*
            The intermediate RLC after `s_main` bytes needs to be properly computed.
            */
            constraints.push((
                "s_main RLC",
                q_not_first.clone()
                    * q_enable.clone()
                    * (one.clone() - is_branch_init_prev.clone())
                    * (rlc - acc_s.clone()),
            ));

            // We use rotation 0 in both cases from now on:
            let c_rlp2 = meta.query_advice(c_main.rlp2, Rotation::cur());
            let c160 = Expression::Constant(F::from(160_u64));

            // c_rlp2 = 160 when branch is hashed (longer than 31) and c_rlp2 = 0 otherwise
            let is_branch_hashed = c_rlp2.clone() * c160_inv.clone();

            /*
            When the branch is hashed, we have `c_rlp2 = 160` because it specifies the length of the
            hash: `32 = 160 - 128`.
            */
            constraints.push((
                "c_rlp2 = 160",
                q_not_first.clone()
                    * q_enable.clone()
                    * is_branch_hashed.clone()
                    * (c_rlp2.clone() - c160),
            ));

            /*
            Note: hashed branch has 160 at c_rlp2 and hash in c_advices,
            non-hashed branch has 0 at c_rlp2 and all the bytes in c_advices
            */

            let acc_mult_s = meta.query_advice(accs.acc_s.mult, Rotation::cur());
            let c_advices0 = meta.query_advice(c_main.bytes[0], Rotation::cur());
            rlc = acc_s.clone() + c_rlp2 * acc_mult_s.clone();
            let c_advices_rlc = compute_rlc(
                meta,
                c_main.bytes.to_vec(),
                0,
                acc_mult_s.clone(),
                0,
                r_table.clone(),
            );
            rlc = rlc + c_advices_rlc;

            let mut rlc_non_hashed_branch = acc_s + c_advices0 * acc_mult_s.clone();
            let c_advices_rlc_non_hashed = compute_rlc(
                meta,
                c_main.bytes.iter().skip(1).copied().collect_vec(),
                0,
                acc_mult_s,
                0,
                r_table,
            );
            rlc_non_hashed_branch = rlc_non_hashed_branch + c_advices_rlc_non_hashed;

            let acc_c = meta.query_advice(accs.acc_c.rlc, Rotation::cur());

            /*
            Check whether the extension node RLC is properly computed.
            The RLC is used to check whether the extension node is a node at the appropriate position
            in the parent node. That means, it is used in a lookup to check whether
            `(extension_node_RLC, node_hash_RLC)` is in the keccak table.
            */
            constraints.push((
                "Hashed extension node RLC",
                q_not_first.clone()
                    * q_enable.clone()
                    * (one.clone() - is_branch_init_prev.clone())
                    * is_branch_hashed.clone()
                    * (rlc - acc_c.clone()),
            ));

            /*
            Check whether the extension node (non-hashed) RLC is properly computed.
            The RLC is used to check whether the non-hashed extension node is a node at the appropriate position
            in the parent node. That means, there is a constraint to ensure that
            `extension_node_RLC = node_hash_RLC` for some `node` in parent branch.
            */
            constraints.push((
                "Non-hashed extension node RLC",
                q_not_first
                    * q_enable
                    * (one.clone() - is_branch_init_prev)
                    * (one.clone() - is_branch_hashed)
                    * (rlc_non_hashed_branch - acc_c),
            ));

            constraints
        });

        meta.create_gate("Extension node selectors & RLP", |meta| {
            let q_not_first = meta.query_fixed(position_cols.q_not_first, Rotation::cur());
            let q_enable = q_enable(meta);
            let mut constraints = vec![];

            // NOTE: even and odd is for number of nibbles that are compactly encoded.

            // To reduce the expression degree, we pack together multiple information.
            let is_ext_short_c16 = meta.query_advice(
                s_main.bytes[IS_EXT_SHORT_C16_POS - RLP_NUM],
                Rotation(rot_into_branch_init),
            );
            let is_ext_short_c1 = meta.query_advice(
                s_main.bytes[IS_EXT_SHORT_C1_POS - RLP_NUM],
                Rotation(rot_into_branch_init),
            );
            let is_ext_long_even_c16 = meta.query_advice(
                s_main.bytes[IS_EXT_LONG_EVEN_C16_POS - RLP_NUM],
                Rotation(rot_into_branch_init),
            );
            let is_ext_long_even_c1 = meta.query_advice(
                s_main.bytes[IS_EXT_LONG_EVEN_C1_POS - RLP_NUM],
                Rotation(rot_into_branch_init),
            );
            let is_ext_long_odd_c16 = meta.query_advice(
                s_main.bytes[IS_EXT_LONG_ODD_C16_POS - RLP_NUM],
                Rotation(rot_into_branch_init),
            );
            let is_ext_long_odd_c1 = meta.query_advice(
                s_main.bytes[IS_EXT_LONG_ODD_C1_POS - RLP_NUM],
                Rotation(rot_into_branch_init),
            );
            let mut is_ext_longer_than_55 = meta.query_advice(
                s_main.bytes[IS_S_EXT_LONGER_THAN_55_POS - RLP_NUM],
                Rotation(rot_into_branch_init),
            );
            if !is_s {
                is_ext_longer_than_55 = meta.query_advice(
                    s_main.bytes[IS_C_EXT_LONGER_THAN_55_POS - RLP_NUM],
                    Rotation(rot_into_branch_init),
                );
            }
            let mut is_ext_node_non_hashed = s_main.bytes[IS_S_EXT_NODE_NON_HASHED_POS - RLP_NUM];
            if !is_s {
                is_ext_node_non_hashed = s_main.bytes[IS_C_EXT_NODE_NON_HASHED_POS - RLP_NUM];
            }
            let is_ext_node_non_hashed =
                meta.query_advice(is_ext_node_non_hashed, Rotation(rot_into_branch_init));

            let is_branch_init_prev = meta.query_advice(branch.is_init, Rotation::prev());

            /*
            We first check that the selectors in branch init row are boolean.

            We have the following selectors in branch init:
            ```
            is_ext_short_c16
            is_ext_short_c1
            is_ext_long_even_c16
            is_ext_long_even_c1
            is_ext_long_odd_c16
            is_ext_long_odd_c1
            ```

            `short` means there is only one nibble in the extension node, `long` means there
            are at least two. `even` means the number of nibbles is even, `odd` means the number
            of nibbles is odd. `c16` means that above the branch there are even number of
            nibbles (the same as saying that `modified_node` of the branch needs to be
            multiplied by 16 in the computation of the key RLC), `c1` means
            that above the branch there are odd number of
            nibbles (the same as saying that `modified_node` of the branch needs to be
            multiplied by 1 in the computation of the key RLC).
            */
            constraints.push((
                "Bool check is_ext_short_c16",
                get_bool_constraint(
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_branch_init_prev.clone()),
                    is_ext_short_c16.clone(),
                ),
            ));
            constraints.push((
                "Bool check is_ext_short_c1",
                get_bool_constraint(
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_branch_init_prev.clone()),
                    is_ext_short_c1.clone(),
                ),
            ));
            constraints.push((
                "Bool check is_ext_long_even_c16",
                get_bool_constraint(
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_branch_init_prev.clone()),
                    is_ext_long_even_c16.clone(),
                ),
            ));
            constraints.push((
                "Bool check is_ext_long_even_c1",
                get_bool_constraint(
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_branch_init_prev.clone()),
                    is_ext_long_even_c1.clone(),
                ),
            ));
            constraints.push((
                "Bool check is_ext_long_odd_c16",
                get_bool_constraint(
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_branch_init_prev.clone()),
                    is_ext_long_odd_c16.clone(),
                ),
            ));
            constraints.push((
                "Bool check is_ext_long_odd_c1",
                get_bool_constraint(
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_branch_init_prev.clone()),
                    is_ext_long_odd_c1.clone(),
                ),
            ));
            constraints.push((
                "Bool check is_ext_longer_than_55",
                get_bool_constraint(
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_branch_init_prev.clone()),
                    is_ext_longer_than_55.clone(),
                ),
            ));
            constraints.push((
                "Bool check is_ext_node_non_hashed",
                get_bool_constraint(
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_branch_init_prev.clone()),
                    is_ext_node_non_hashed,
                ),
            ));

            /*
            Only one of the six options can appear. When we have an extension node it holds:
            `is_ext_short_c16 + is_ext_short_c1 + is_ext_long_even_c16 + is_ext_long_even_c1 + is_ext_long_odd_c16 + is_ext_long_odd_c1 = 1`.
            And when it is a regular branch:
            `is_ext_short_c16 + is_ext_short_c1 + is_ext_long_even_c16 + is_ext_long_even_c1 + is_ext_long_odd_c16 + is_ext_long_odd_c1 = 0`.

            Note that if the attacker sets `is_extension_node = 1`
            for a regular branch (or `is_extension_node = 0` for the extension node),
            the final key RLC check fails because key RLC is computed differently
            for extension nodes and regular branches - a regular branch occupies only one
            key nibble (`modified_node`), while extension node occupies at least one additional
            nibble (the actual extension of the extension node).
            */
            constraints.push((
                "Bool check extension node selectors sum",
                get_bool_constraint(
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_branch_init_prev.clone()),
                    is_ext_short_c16.clone()
                        + is_ext_short_c1.clone()
                        + is_ext_long_even_c16.clone()
                        + is_ext_long_even_c1.clone()
                        + is_ext_long_odd_c16.clone()
                        + is_ext_long_odd_c1.clone(),
                ),
            ));
 
            let is_branch_c16 = meta.query_advice(
                s_main.bytes[IS_BRANCH_C16_POS - RLP_NUM],
                Rotation(rot_into_branch_init),
            );
            let is_branch_c1 = meta.query_advice(
                s_main.bytes[IS_BRANCH_C1_POS - RLP_NUM],
                Rotation(rot_into_branch_init),
            );

            /*
            `is_branch_c16` and `is_branch_c1` information is duplicated with
            extension node selectors when we have an extension node (while in case of a regular
            branch the extension node selectors do not hold this information).
            That means when we have an extension node and `is_branch_c16 = 1`,
            there is `is_ext_short_c16 = 1` or
            `is_ext_long_even_c16 = 1` or `is_ext_long_odd_c16 = 1`.

            We have such a duplication to reduce the expression degree - for example instead of
            using `is_ext_long_even * is_branch_c16` we just use `is_ext_long_even_c16`.

            But we need to check that `is_branch_c16` and `is_branch_c1` are consistent
            with extension node selectors.
            */
            let mut constrain_sel = |branch_sel: Expression<F>, ext_sel: Expression<F>| {
                constraints.push((
                    "Branch c16/c1 selector - extension c16/c1 selector",
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_branch_init_prev.clone())
                        * ext_sel.clone()
                        * (branch_sel - ext_sel),
                ));
            };

            constrain_sel(is_branch_c16.clone(), is_ext_short_c16.clone());
            constrain_sel(is_branch_c1.clone(), is_ext_short_c1.clone());
            constrain_sel(is_branch_c16.clone(), is_ext_long_even_c16.clone());
            constrain_sel(is_branch_c1.clone(), is_ext_long_even_c1.clone());
            constrain_sel(is_branch_c16, is_ext_long_odd_c16.clone());
            constrain_sel(is_branch_c1, is_ext_long_odd_c1.clone());
 
            // In C we have nibbles, we check below only for S.
            if is_s {
                let s_rlp1 = meta.query_advice(s_main.rlp1, Rotation::cur());
                let s_bytes0 = meta.query_advice(s_main.bytes[0], Rotation::cur());

                let is_short = is_ext_short_c16 + is_ext_short_c1;
                let is_even_nibbles = is_ext_long_even_c16 + is_ext_long_even_c1;
                let is_long_odd_nibbles = is_ext_long_odd_c16 + is_ext_long_odd_c1;

                /*
                This constraint prevents the attacker to set the number of nibbles to be even
                when it is not even.
                Note that when it is not even it holds `s_bytes0 != 0` (hexToCompact adds 16).

                If the number of nibbles is 1, like in
                `[226,16,160,172,105,12...`
                there is no byte specifying the length.
                If the number of nibbles is bigger than 1 and it is even, like in
                `[228,130,0,149,160,114,253,150,133,18,192,156,19,241,162,51,210,24,1,151,16,48,7,177,42,60,49,34,230,254,242,79,132,165,90,75,249]`
                the second byte (`s_main.rlp2`) specifies the length (we need to subract 128 to get it),
                the third byte (`s_main.bytes[0]`) is 0.
                */
                constraints.push((
                    "Long & even implies s_bytes0 = 0",
                    q_not_first.clone()
                        * q_enable.clone()
                        * is_even_nibbles.clone()
                        * s_bytes0.clone(),
                ));

                let c_rlp2 = meta.query_advice(c_main.rlp2, Rotation::cur());
                let is_branch_hashed = c_rlp2 * c160_inv.clone();

                /*
                We need to check that the length specified in `s_main.rlp1` corresponds to the actual
                length of the extension node.

                For example, in
                `[226,16,160,172,105,12...`
                we check that `226 - 192 = 1 + 32 + 1`.
                1 is for `s_main.rlp2`, 32 is for 32 bytes of the branch hash,
                1 is for the byte 160 which denotes the length
                of the hash (128 + 32).
                */
                constraints.push((
                    "One nibble & hashed branch RLP",
                    q_not_first.clone()
                        * q_enable.clone()
                        // when one nibble, extension node cannot be longer that 55
                        * is_short.clone()
                        * is_branch_hashed.clone()
                        * (s_rlp1.clone() - c192.clone() - c33.clone() - one.clone()),
                ));

                let c_bytes0 = meta.query_advice(c_main.bytes[0], Rotation::cur());

                /*
                We need to check that the length specified in `s_main.rlp1` corresponds to the actual
                length of the extension node.
                
                For example, in
                `[223,16,221,198,132,32,0,0,0,1,198,132,32,0,0,0,1,128,128,128,128,128,128,128,128,128,128,128,128,128,128,128]`
                we check that `223 - 192 = 1 + 29 + 1`.
                1 is for `s_main.rlp2`,
                29 is for the branch RLP (which is not hashed because it is shorter than 32 bytes),
                1 is for `c_main.bytes[0]` which denotes the length of the branch RLP.
                */
                // TODO: prepare test
                constraints.push((
                    "One nibble & non-hashed branch RLP",
                    q_not_first.clone()
                        * q_enable.clone()
                        // when one nibble, extension node cannot be longer that 55
                        * is_short
                        * (one.clone() - is_branch_hashed.clone())
                        * (s_rlp1.clone() - c192.clone() - one.clone() - (c_bytes0.clone() - c192.clone()) - one.clone()),
                ));

                let s_rlp2 = meta.query_advice(s_main.rlp2, Rotation::cur());

                /*
                We need to check that the length specified in `s_main.rlp1` corresponds to the actual
                length of the extension node.
                
                For example, in
                `[228,130,0,149,160,114,253...`
                we check that `228 - 192 = (130 - 128) + 1 + 32 + 1`.
                1 is for `s_main.rlp2` which specifies the length of the nibbles part,
                32 is for the branch hash,
                1 is for the byte 160 which denotes the length
                of the hash (128 + 32).
                */
                constraints.push((
                    "More than one nibble & hashed branch & ext not longer than 55 RLP",
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_ext_longer_than_55.clone())
                        * (is_even_nibbles.clone() + is_long_odd_nibbles.clone())
                        * is_branch_hashed.clone()
                        * (s_rlp1.clone() - c192.clone() - (s_rlp2.clone() - c128.clone()) - one.clone() - c33.clone()),
                ));

                /*
                We need to check that the length specified in `s_main.rlp1` corresponds to the actual
                length of the extension node.
                
                We check that `s_main.rlp1 - 192` = `s_main.rlp2 - 128 + 1 + c_main.bytes[0] - 192 + 1`.
                */
                constraints.push((
                    "More than one nibble & non-hashed branch & ext not longer than 55 RLP",
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - is_ext_longer_than_55.clone())
                        * (is_even_nibbles + is_long_odd_nibbles)
                        * (one.clone() - is_branch_hashed.clone())
                        * (s_rlp1.clone() - c192.clone() - (s_rlp2.clone() - c128.clone()) - one.clone()
                            - (c_bytes0.clone() - c192.clone()) - one.clone()),
                ));

                /*
                Note: ext longer than 55 RLP cannot appear when there is only one nibble because in this case
                we would have 1 byte for a nibble and at most 32 bytes for branch.
                */

                /*
                When extension node RLP is longer than 55 bytes, the RLP has an additional byte
                at second position and the first byte specifies the length of the substream
                that specifies the length of the RLP. The substream is always just one byte: `s_main.rlp2`.
                And `s_main.rlp1 = 248` where `248 = 247 + 1` means the length of 1 byte.

                Example:
                `[248,67,160,59,138,106,70,105,186,37,13,38,205,122,69,158,202,157,33,95,131,7,227,58,235,229,3,121,188,90,54,23,236,52,68,161,160,...`
                */
                constraints.push((
                    "Extension longer than 55 RLP: s_rlp1 = 248",
                    q_not_first.clone()
                        * q_enable.clone()
                        * is_ext_longer_than_55.clone()
                        * (s_rlp1.clone() - c248.clone()),
                ));

                /*
                We need to check that the length specified in `s_main.rlp2` corresponds to the actual
                length of the extension node.

                Example:
                `[248,67,160,59,138,106,70,105,186,37,13,38,205,122,69,158,202,157,33,95,131,7,227,58,235,229,3,121,188,90,54,23,236,52,68,161,160,...`
                
                We check that `s_main.rlp2 = (s_main.bytes[0] - 128) + 1 + 32 + 1`.
                `s_main.bytes[0] - 128` specifies the extension node nibbles part, 
                1 is for `s_main.rlp2` which specifies the length of the RLP stream,
                32 is for the branch hash,
                1 is for the byte 160 which denotes the length of the hash (128 + 32). 
                */
                // TODO: test
                constraints.push((
                    "Hashed branch & ext longer than 55 RLP",
                    q_not_first.clone()
                        * q_enable.clone()
                        * is_ext_longer_than_55.clone()
                        * is_branch_hashed.clone()
                        * (s_rlp2 - (s_bytes0.clone() - c128.clone()) - one.clone() - c33.clone()),
                ));

                /*
                We need to check that the length specified in `s_main.rlp2` corresponds to the actual
                length of the extension node.

                We check that `s_main.rlp2 = (s_main.bytes[0] - 128) + 1 + c_main.bytes[0] - 192 + 1`.
                `s_main.bytes[0] - 128` specifies the extension node nibbles part, 
                1 is for `s_main.rlp2` which specifies the length of the RLP stream,
                `c_main.bytes[0] - 192` is for the branch RLP (which is not hashed because it is shorter than 32 bytes),
                1 is for the byte 160 which denotes the length of the hash (128 + 32). 
                */
                // TODO: test
                constraints.push((
                    "Non-hashed branch & ext longer than 55 RLP",
                    q_not_first
                        * q_enable
                        * is_ext_longer_than_55
                        * (one.clone() - is_branch_hashed)
                        * (s_rlp1 - (s_bytes0 - c128.clone()) - one.clone()
                            - (c_bytes0 - c192.clone()) - one.clone()),
                ));

                /*
                Some observations:

                [228,130,0,149,160,114,253,150,133,18,192,156,19,241,162,51,210,24,1,151,16,48,7,177,42,60,49,34,230,254,242,79,132,165,90,75,249]
                Note that the first element (228 in this case) can go much higher - for example, if there
                are 40 nibbles, this would take 20 bytes which would make the first element 248.

                If only one byte in key:
                [226,16,160,172,105,12...

                Extension node with non-hashed branch:
                List contains up to 55 bytes (192 + 55)
                [247,160,16,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,213,128,194,32,1,128,194,32,1,128,128,128,128,128,128,128,128,128,128,128,128,128]

                List contains more than 55 bytes
                [248,58,159,16,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,217,128,196,130,32,0,1,128,196,130,32,0,1,128,128,128,128,128,128,128,128,128,128,128,128,128]

                Note that the extension node can be much shorter than the one above - in case when
                there are less nibbles, so we cannot say that 226 appears as the first byte only
                when there are hashed nodes in the branch and there is only one nibble.
                Branch with two non-hashed nodes (that's the shortest possible branch):
                [217,128,196,130,32,0,1,128,196,130,32,0,1,128,128,128,128,128,128,128,128,128,128,128,128,128]
                Note: branch contains at least 26 bytes. 192 + 26 = 218

                If proofEl[0] <= 247 (length at most 55, so proofEl[1] doesn't specify the length of the whole
                    remaining stream, only of the next substream)
                If proofEl[1] <= 128:
                    There is only 1 byte for nibbles (keyLen = 1) and this is proofEl[1].
                Else:
                    Nibbles are stored in more than 1 byte, proofEl[1] specifies the length of bytes.
                Else:
                proofEl[1] contains the length of the remaining stream.
                proofEl[2] specifies the length of the bytes (for storing nibbles).
                Note that we can't have only one nibble in this case.
                */
            }

            constraints
        });

        // Note: acc_mult is checked in `extension_node_key.rs`.

        /*
        Check whether branch hash is in the extension node row - we check that the branch hash RLC
        (computed over the first 17 rows) corresponds to the extension node hash stored in
        the extension node row. That means `(branch_RLC, extension_node_hash_RLC`) needs to
        be in a keccak table.
        */
        meta.lookup_any("Extension node branch hash in extension row", |meta| {
            let q_enable = q_enable(meta);
            let q_not_first = meta.query_fixed(position_cols.q_not_first, Rotation::cur());
            let is_branch_init_prev = meta.query_advice(branch.is_init, Rotation::prev());

            let c_rlp2 = meta.query_advice(c_main.rlp2, Rotation::cur());
            let is_branch_hashed = c_rlp2 * c160_inv.clone();

            let mut acc = meta.query_advice(accs.acc_s.rlc, Rotation(-1));
            let mut mult = meta.query_advice(accs.acc_s.mult, Rotation(-1));
            if !is_s {
                acc = meta.query_advice(accs.acc_c.rlc, Rotation(-2));
                mult = meta.query_advice(accs.acc_c.mult, Rotation(-2));
            }
            // TODO: acc currently doesn't have branch ValueNode info (which 128 if nil)
            let branch_acc = acc + c128.clone() * mult;

            let mut constraints = vec![(
                q_not_first.clone()
                    * q_enable.clone()
                    * (one.clone() - is_branch_init_prev.clone())
                    * is_branch_hashed.clone()
                    * branch_acc, // TODO: replace with acc once ValueNode is added
                meta.query_fixed(keccak_table[0], Rotation::cur()),
            )];

            let mut sc_hash = vec![];
            // Note: extension node has branch hash always in c_advices.
            for column in c_main.bytes.iter() {
                sc_hash.push(meta.query_advice(*column, Rotation::cur()));
            }
            let hash_rlc = bytes_expr_into_rlc(&sc_hash, acc_r);
            constraints.push((
                q_not_first
                    * q_enable
                    * (one.clone() - is_branch_init_prev)
                    * is_branch_hashed
                    * hash_rlc,
                meta.query_fixed(keccak_table[1], Rotation::cur()),
            ));

            constraints
        });

        meta.create_gate(
            "Extension node branch hash in extension row (non-hashed branch)",
            |meta| {
                let mut constraints = vec![];
                let q_not_first = meta.query_fixed(position_cols.q_not_first, Rotation::cur());
                let q_enable = q_enable(meta);

                let c_rlp2 = meta.query_advice(c_main.rlp2, Rotation::cur());
                // c_rlp2 = 160 when branch is hashed (longer than 31) and c_rlp2 = 0 otherwise
                let is_branch_hashed = c_rlp2 * c160_inv.clone();

                let mut acc = meta.query_advice(accs.acc_s.rlc, Rotation(-1));
                let mut mult = meta.query_advice(accs.acc_s.mult, Rotation(-1));
                if !is_s {
                    acc = meta.query_advice(accs.acc_c.rlc, Rotation(-2));
                    mult = meta.query_advice(accs.acc_c.mult, Rotation(-2));
                }
                // TODO: acc currently doesn't have branch ValueNode info (which 128 if nil)
                let branch_acc = acc + c128.clone() * mult;

                let mut branch_in_ext = vec![];
                // Note: extension node has branch hash always in c_advices.
                for column in c_main.bytes.iter() {
                    branch_in_ext.push(meta.query_advice(*column, Rotation::cur()));
                }
                let rlc = bytes_expr_into_rlc(&branch_in_ext, acc_r);

                /*
                Check whether branch is in extension node row (non-hashed branch) -
                we check that the branch RLC is the same as the extension node branch part RLC
                (RLC computed over `c_main.bytes`).

                Note: there need to be 0s after branch ends in the extension node `c_main.bytes`
                (see below).
                */
                constraints.push((
                    "Non-hashed branch in extension node",
                    q_not_first * q_enable * (one.clone() - is_branch_hashed) * (branch_acc - rlc),
                ));

                constraints
            },
        );

        let _sel_branch_non_hashed = |meta: &mut VirtualCells<F>| {
            let q_not_first = meta.query_fixed(position_cols.q_not_first, Rotation::cur());
            let q_enable = q_enable(meta);

            let c_rlp2 = meta.query_advice(c_main.rlp2, Rotation::cur());
            // c_rlp2 = 160 when branch is hashed (longer than 31) and c_rlp2 = 0 otherwise
            let is_branch_hashed = c_rlp2 * c160_inv.clone();

            q_not_first * q_enable * (one.clone() - is_branch_hashed)
        };

        /*
        /*
        There are 0s after non-hashed branch ends in `c_main.bytes`.
        */
        for ind in 1..HASH_WIDTH {
            key_len_lookup(
                meta,
                sel_branch_non_hashed,
                ind,
                c_main.bytes[0],
                c_main.bytes[ind],
                192,
                fixed_table,
            )
        }
        */

        /*
        Note: Correspondence between nibbles in C and bytes in S is checked in
        extension_node_key.
        */

        /*
        When we have an extension node in the first level of the account trie,
        its hash needs to be compared to the root of the trie.

        Note: the branch counterpart is implemented in `branch_hash_in_parent.rs`.
        */
        meta.lookup_any(
            "Account first level extension node hash - compared to root",
            |meta| {
                let q_enable = q_enable(meta);
                let mut constraints = vec![];

                let q_not_first = meta.query_fixed(position_cols.q_not_first, Rotation::cur());
                let not_first_level = meta.query_advice(position_cols.not_first_level, Rotation::cur());

                let acc_c = meta.query_advice(accs.acc_c.rlc, Rotation::cur());
                let root = meta.query_advice(inter_root, Rotation::cur());

                constraints.push((
                    q_not_first.clone()
                        * q_enable.clone()
                        * (one.clone() - not_first_level.clone())
                        * acc_c,
                    meta.query_fixed(keccak_table[0], Rotation::cur()),
                ));
                let keccak_table_i = meta.query_fixed(keccak_table[1], Rotation::cur());
                constraints.push((
                    q_not_first * q_enable * (one.clone() - not_first_level) * root,
                    keccak_table_i,
                ));

                constraints
            },
        );

        /*
        When extension node is in the first level of the storage trie, we need to check whether
        `hash(ext_node) = storage_trie_root`. We do this by checking whether
        `(ext_node_RLC, storage_trie_root_RLC)` is in the keccak table.

        Note: extension node in the first level cannot be shorter than 32 bytes (it is always hashed).
        */
        meta.lookup_any(
            "Extension node in first level of storage trie - hash compared to the storage root",
            |meta| {
                let not_first_level = meta.query_advice(position_cols.not_first_level, Rotation::cur());

                let mut rot_into_branch_init = -17;
                let mut rot_into_last_branch_child = -1;
                let mut is_branch_placeholder = meta.query_advice(
                    s_main.bytes[IS_BRANCH_S_PLACEHOLDER_POS - RLP_NUM],
                    Rotation(rot_into_branch_init),
                );
                if !is_s {
                    rot_into_branch_init = -18;
                    rot_into_last_branch_child = -2;
                    is_branch_placeholder = meta.query_advice(
                        s_main.bytes[IS_BRANCH_C_PLACEHOLDER_POS - RLP_NUM],
                        Rotation(rot_into_branch_init),
                    );
                }

                // Only check if there is an account above the leaf.
                let is_account_leaf_in_added_branch = meta.query_advice(
                    is_account_leaf_in_added_branch,
                    Rotation(rot_into_branch_init - 1),
                );

                let is_extension_node =
                    get_is_extension_node(meta, s_main.bytes, rot_into_branch_init);

                // We need to do the lookup only if we are in the last branch child.
                let is_after_last_branch_child =
                    meta.query_advice(branch.is_last_child, Rotation(rot_into_last_branch_child));

                // Note: acc_c in both cases.
                let acc = meta.query_advice(accs.acc_c.rlc, Rotation::cur());

                let mut sc_hash = vec![];
                // Note: storage root is always in `s_main.bytes`.
                for column in s_main.bytes.iter() {
                    if is_s {
                        sc_hash.push(meta.query_advice(
                            *column,
                            Rotation(
                                rot_into_branch_init
                                    - (ACCOUNT_LEAF_ROWS - ACCOUNT_LEAF_STORAGE_CODEHASH_S_IND),
                            ),
                        ));
                    } else {
                        sc_hash.push(meta.query_advice(
                            *column,
                            Rotation(
                                rot_into_branch_init
                                    - (ACCOUNT_LEAF_ROWS - ACCOUNT_LEAF_STORAGE_CODEHASH_C_IND),
                            ),
                        ));
                    }
                }
                let hash_rlc = bytes_expr_into_rlc(&sc_hash, acc_r);

                let mut constraints = vec![(
                    not_first_level.clone()
                        * is_extension_node.clone()
                        * is_after_last_branch_child.clone()
                        * is_account_leaf_in_added_branch.clone()
                        * (one.clone() - is_branch_placeholder.clone())
                        * acc,
                    meta.query_fixed(keccak_table[0], Rotation::cur()),
                )];
                constraints.push((
                    not_first_level
                        * is_extension_node
                        * is_after_last_branch_child
                        * is_account_leaf_in_added_branch
                        * (one.clone() - is_branch_placeholder)
                        * hash_rlc,
                    meta.query_fixed(keccak_table[1], Rotation::cur()),
                ));

                constraints
            },
        );

        /*
        Check whether the extension node hash is in the parent branch.
        That means we check whether
        `(extension_node_RLC, node_hash_RLC)` is in the keccak table where `node` is a parent
        brach child at `modified_node` position.

        Note: do not check if it is in the first storage level (see `storage_root_in_account_leaf.rs`).
        */
        meta.lookup_any("Extension node hash in parent branch", |meta| {
            let q_enable = q_enable(meta);
            let not_first_level = meta.query_advice(position_cols.not_first_level, Rotation::cur());

            let is_account_leaf_in_added_branch = meta.query_advice(
                is_account_leaf_in_added_branch,
                Rotation(rot_into_branch_init - 1),
            );

            // When placeholder extension, we don't check its hash in a parent.
            let mut is_branch_placeholder = s_main.bytes[IS_BRANCH_S_PLACEHOLDER_POS - RLP_NUM];
            if !is_s {
                is_branch_placeholder = s_main.bytes[IS_BRANCH_C_PLACEHOLDER_POS - RLP_NUM];
            }
            let is_branch_placeholder =
                meta.query_advice(is_branch_placeholder, Rotation(rot_into_branch_init));

            let mut is_ext_node_non_hashed = s_main.bytes[IS_S_EXT_NODE_NON_HASHED_POS - RLP_NUM];
            if !is_s {
                is_ext_node_non_hashed = s_main.bytes[IS_C_EXT_NODE_NON_HASHED_POS - RLP_NUM];
            }
            let is_ext_node_non_hashed =
                meta.query_advice(is_ext_node_non_hashed, Rotation(rot_into_branch_init));

            let mut constraints = vec![];

            let acc_c = meta.query_advice(accs.acc_c.rlc, Rotation::cur());
            constraints.push((
                not_first_level.clone()
                    * q_enable.clone()
                    * (one.clone() - is_account_leaf_in_added_branch.clone())
                    * (one.clone() - is_branch_placeholder.clone())
                    * (one.clone() - is_ext_node_non_hashed.clone())
                    * acc_c,
                meta.query_fixed(keccak_table[0], Rotation::cur()),
            ));

            // Any rotation that lands into branch can be used instead of -21.
            let mut mod_node_hash_rlc_cur = meta.query_advice(accs.s_mod_node_rlc, Rotation(-21));
            if !is_s {
                mod_node_hash_rlc_cur = meta.query_advice(accs.c_mod_node_rlc, Rotation(-21));
            }

            let keccak_table_i = meta.query_fixed(keccak_table[1], Rotation::cur());
            constraints.push((
                not_first_level
                    * q_enable
                    * (one.clone() - is_account_leaf_in_added_branch)
                    * (one.clone() - is_branch_placeholder)
                    * (one.clone() - is_ext_node_non_hashed)
                    * mod_node_hash_rlc_cur,
                keccak_table_i,
            ));

            constraints
        });

        meta.create_gate(
            "Extension node in parent branch (non-hashed extension node)",
            |meta| {
                let q_enable = q_enable(meta);
                let q_not_first = meta.query_fixed(position_cols.q_not_first, Rotation::cur());
                let not_first_level = meta.query_advice(position_cols.not_first_level, Rotation::cur());

                let is_account_leaf_in_added_branch = meta.query_advice(
                    is_account_leaf_in_added_branch,
                    Rotation(rot_into_branch_init - 1),
                );

                // When placeholder extension, we don't check its hash in a parent.
                let mut is_branch_placeholder = s_main.bytes[IS_BRANCH_S_PLACEHOLDER_POS - RLP_NUM];
                if !is_s {
                    is_branch_placeholder = s_main.bytes[IS_BRANCH_C_PLACEHOLDER_POS - RLP_NUM];
                }
                let is_branch_placeholder =
                    meta.query_advice(is_branch_placeholder, Rotation(rot_into_branch_init));

                let mut is_ext_node_non_hashed =
                    s_main.bytes[IS_S_EXT_NODE_NON_HASHED_POS - RLP_NUM];
                if !is_s {
                    is_ext_node_non_hashed = s_main.bytes[IS_C_EXT_NODE_NON_HASHED_POS - RLP_NUM];
                }
                let is_ext_node_non_hashed =
                    meta.query_advice(is_ext_node_non_hashed, Rotation(rot_into_branch_init));

                let mut constraints = vec![];

                let acc_c = meta.query_advice(accs.acc_c.rlc, Rotation::cur());
                let mut mod_node_hash_rlc_cur =
                    meta.query_advice(accs.s_mod_node_rlc, Rotation(-21));
                if !is_s {
                    mod_node_hash_rlc_cur = meta.query_advice(accs.c_mod_node_rlc, Rotation(-21));
                }

                /*
                When an extension node is not hashed, we do not check whether it is in a parent
                branch using a lookup (see above), instead we need to check whether the branch child
                at `modified_node` position is exactly the same as the extension node.
                */
                constraints.push((
                    "Non-hashed extension node in parent branch",
                    q_not_first
                        * not_first_level
                        * q_enable
                        * (one.clone() - is_account_leaf_in_added_branch)
                        * (one.clone() - is_branch_placeholder)
                        * is_ext_node_non_hashed
                        * (mod_node_hash_rlc_cur - acc_c),
                ));

                constraints
            },
        );

        /*
        We need to make sure the total number of nibbles is 64. This constraint ensures the number
        of nibbles used (stored in branch init) is correctly computed - nibbles up until this
        extension node + nibbles in this extension node.
        Once in a leaf, the remaining nibbles stored in a leaf need to be added to the count.
        The final count needs to be 64.
        */
        if is_s {
            meta.create_gate("Extension node number of nibbles", |meta| {
                let mut constraints = vec![];
                let q_not_first = meta.query_fixed(position_cols.q_not_first, Rotation::cur());
                let q_enable = q_enable(meta);
                let not_first_level = meta.query_advice(position_cols.not_first_level, Rotation::cur());

                // Only check if there is an account above the branch.
                let is_first_storage_level = meta.query_advice(
                    is_account_leaf_in_added_branch,
                    Rotation(rot_into_branch_init - 1),
                );

                let is_ext_longer_than_55 = meta.query_advice(
                    s_main.bytes[IS_S_EXT_LONGER_THAN_55_POS - RLP_NUM],
                    Rotation(rot_into_branch_init),
                );

                let is_short =
                    get_is_extension_node_one_nibble(meta, s_main.bytes, rot_into_branch_init);
                let is_even_nibbles =
                    get_is_extension_node_even_nibbles(meta, s_main.bytes, rot_into_branch_init);
                let is_long_odd_nibbles = get_is_extension_node_long_odd_nibbles(
                    meta,
                    s_main.bytes,
                    rot_into_branch_init,
                );

                /*
                Note: for regular branches, the constraint that `nibbles_count` increases
                by 1 is in `branch.rs`.
                */

                let nibbles_count_cur = meta.query_advice(
                    s_main.bytes[NIBBLES_COUNTER_POS - RLP_NUM],
                    Rotation(rot_into_branch_init),
                );
                let mut nibbles_count_prev = meta.query_advice(
                    s_main.bytes[NIBBLES_COUNTER_POS - RLP_NUM],
                    Rotation(rot_into_branch_init - BRANCH_ROWS_NUM),
                );

                /*
                nibbles_count_prev needs to be 0 when in first account level or
                in first storage level
                */
                nibbles_count_prev =
                    (one.clone() - is_first_storage_level) * nibbles_count_prev.clone();
                nibbles_count_prev = not_first_level * nibbles_count_prev.clone();

                /*
                When there is only one nibble in the extension node, `nibbles_count` changes
                for 2: one nibble and `modified_node` in a branch.
                */
                constraints.push((
                    "Nibbles number when one nibble",
                    q_not_first.clone()
                        * q_enable.clone()
                        * is_short
                        * (nibbles_count_cur.clone()
                            - nibbles_count_prev.clone()
                            - one.clone()
                            - one.clone()), // -1 for nibble, - 1 is for branch position
                ));

                let s_rlp2 = meta.query_advice(s_main.rlp2, Rotation::cur());
                let mut num_nibbles =
                    (s_rlp2.clone() - c128.clone() - one.clone()) * (one.clone() + one.clone());
                /*
                When there is an even number of nibbles in the extension node,
                we compute the number of nibbles as: `(s_rlp2 - 128 - 1) * 2`.
                By `s_rlp2 - 128` we get the number of bytes where nibbles are compressed, but
                then we need to subtract 1 because `s_main.bytes[0]` does not contain any nibbles
                (it is just 0 when even number of nibbles).

                In the example below it is: `(130 - 128 - 1) * 2`.
                `[228,130,0,149,160,114,253...`
                */
                constraints.push((
                    "Nibbles number when even number of nibbles & ext not longer than 55",
                    q_not_first.clone()
                        * q_enable.clone()
                        * is_even_nibbles.clone()
                        * (one.clone() - is_ext_longer_than_55.clone())
                        * (nibbles_count_cur.clone()
                            - nibbles_count_prev.clone()
                            - num_nibbles.clone()
                            - one.clone()), // - 1 is for branch position
                ));

                num_nibbles = (s_rlp2 - c128.clone()) * (one.clone() + one.clone()) - one.clone();

                /*
                When there is an odd number of nibbles in the extension node,
                we compute the number of nibbles as: `(s_rlp2 - 128) * 2`.
                By `s_rlp2 - 128` we get the number of bytes where nibbles are compressed. We
                multiply by 2 to get the nibbles, but then subtract 1 because in
                `s_main.bytes[0]` there is only 1 nibble.
                */
                constraints.push((
                    "Nibbles num when odd number (>1) of nibbles & ext not longer than 55",
                    q_not_first.clone()
                        * q_enable.clone()
                        * is_long_odd_nibbles.clone()
                        * (one.clone() - is_ext_longer_than_55.clone())
                        * (nibbles_count_cur.clone()
                            - nibbles_count_prev.clone()
                            - num_nibbles.clone()
                            - one.clone()), // - 1 is for branch position
                ));

                let s_bytes0 = meta.query_advice(s_main.bytes[0], Rotation::cur());
                num_nibbles =
                    (s_bytes0.clone() - c128.clone() - one.clone()) * (one.clone() + one.clone());

                /*
                When there is an even number of nibbles in the extension node and the extension
                node is longer than 55 bytes, the number of bytes containing the nibbles
                is given by `s_main.bytes[0]`.
                We compute the number of nibbles as: `(s_bytes0 - 128 - 1) * 2`.
                By `s_bytes0 - 128` we get the number of bytes where nibbles are compressed, but
                then we need to subtract 1 because `s_main.bytes[1]` does not contain any nibbles
                (it is just 0 when even number of nibbles).
                */
                constraints.push((
                    "Nibbles num when even number of nibbles & ext longer than 55",
                    q_not_first.clone()
                        * q_enable.clone()
                        * is_even_nibbles
                        * is_ext_longer_than_55.clone()
                        * (nibbles_count_cur.clone()
                            - nibbles_count_prev.clone()
                            - num_nibbles.clone()
                            - one.clone()), // - 1 is for branch position
                ));

                num_nibbles = (s_bytes0 - c128.clone()) * (one.clone() + one.clone()) - one.clone();

                /*
                When there is an odd number of nibbles in the extension node and the extension,
                node is longer than 55 bytes, the number of bytes containing the nibbles
                is given by `s_main.bytes[0]`.
                We compute the number of nibbles as: `(s_main.bytes[0] - 128) * 2`.
                By `s_main.bytes[0] - 128` we get the number of bytes where nibbles are compressed. We
                multiply by 2 to get the nibbles, but then subtract 1 because in
                `s_main.bytes[1]` there is only 1 nibble.

                Example:
                `[248,58,159,16,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,217,128,196,130,32,0,1,128,196,130,32,0,1,128,128,128,128,128,128,128,128,128,128,128,128,128]`
                */
                constraints.push((
                    "Nibbles num when odd number (>1) of nibbles & ext longer than 55",
                    q_not_first
                        * q_enable
                        * is_long_odd_nibbles
                        * is_ext_longer_than_55
                        * (nibbles_count_cur - nibbles_count_prev - num_nibbles - one), // - 1 is for branch position
                ));

                constraints
            });
        }

        // Note: range_lookups are in extension_node_key.

        config
    }

    pub fn assign(
        &self,
        region: &mut Region<'_, F>,
        mpt_config: &MPTConfig<F>,
        pv: &mut ProofValues<F>,
        row: &MptWitnessRow<F>,
        offset: usize,
        is_s: bool,
    ) {
        if pv.is_extension_node {
            if is_s {
                // [228,130,0,149,160,114,253,150,133,18,192,156,19,241,162,51,210,24,1,151,16,
                // 48,7,177,42,60,49,34,230,254,242,79,132,165,90,75,249]

                // One nibble:
                // [226,16,160,172,105,12...
                // Could also be non-hashed branch:
                // [223,16,221,198,132,32,0,0,0,1,198,132,32,0,0,0,1,128,128,128,128,128,128,
                // 128,128,128,128,128,128,128,128,128]

                // [247,160,16,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
                // 213,128,194,32,1,128,194,32,1,128,128,128,128,128,128,128,128,128,128,128,
                // 128,128] [248,58,159,16,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
                // 0,0,0,0,0,0,0,0,0,0,0,0,217,128,196,130,32,0,1,128,196,130,32,0,1,128,128,
                // 128,128,128,128,128,128,128,128,128,128,128]

                // Intermediate RLC value and mult (after key)
                // to know which mult we need to use in c_advices.
                pv.acc_s = F::zero();
                pv.acc_mult_s = F::one();
                let len: usize;
                if row.get_byte(1) <= 32 {
                    // key length is 1
                    len = 2 // [length byte, key]
                } else if row.get_byte(0) < 248 {
                    len = (row.get_byte(1) - 128) as usize + 2;
                } else {
                    len = (row.get_byte(2) - 128) as usize + 3;
                }
                mpt_config.compute_acc_and_mult(
                    &row.bytes,
                    &mut pv.acc_s,
                    &mut pv.acc_mult_s,
                    0,
                    len,
                );

                // Final RLC value.
                pv.acc_c = pv.acc_s;
                pv.acc_mult_c = pv.acc_mult_s;
                let mut start = C_RLP_START + 1;
                let mut len = HASH_WIDTH + 1;
                if row.get_byte(C_RLP_START + 1) == 0 {
                    // non-hashed branch in extension node
                    start = C_START;
                    len = HASH_WIDTH;
                }
                mpt_config.compute_acc_and_mult(
                    &row.bytes,
                    &mut pv.acc_c,
                    &mut pv.acc_mult_c,
                    start,
                    len,
                );

                mpt_config
                    .assign_acc(region, pv.acc_s, pv.acc_mult_s, pv.acc_c, F::zero(), offset)
                    .ok();
            } else {
                // We use intermediate value from previous row (because
                // up to acc_s it's about key and this is the same
                // for both S and C).
                pv.acc_c = pv.acc_s;
                pv.acc_mult_c = pv.acc_mult_s;
                let mut start = C_RLP_START + 1;
                let mut len = HASH_WIDTH + 1;
                if row.get_byte(C_RLP_START + 1) == 0 {
                    // non-hashed branch in extension node
                    start = C_START;
                    len = HASH_WIDTH;
                }
                mpt_config.compute_acc_and_mult(
                    &row.bytes,
                    &mut pv.acc_c,
                    &mut pv.acc_mult_c,
                    start,
                    len,
                );

                mpt_config
                    .assign_acc(region, pv.acc_s, pv.acc_mult_s, pv.acc_c, F::zero(), offset)
                    .ok();
            }
            region
                .assign_advice(
                    || "assign key_rlc".to_string(),
                    mpt_config.accumulators.key.rlc,
                    offset,
                    || Ok(pv.extension_node_rlc),
                )
                .ok();
        }
    }
}

use halo2_proofs::{
    circuit::{Region, Value},
    plonk::{Advice, Column, ConstraintSystem, Expression, Fixed, VirtualCells},
    poly::Rotation,
    arithmetic::FieldExt,
};
use std::marker::PhantomData;

use crate::{
    mpt_circuit::columns::{AccumulatorCols, MainCols},
    mpt_circuit::helpers::range_lookups,
    mpt_circuit::{FixedTableTag, MPTConfig, param::IS_NON_EXISTING_ACCOUNT_POS},
    mpt_circuit::param::{
        ACCOUNT_NON_EXISTING_IND, BRANCH_ROWS_NUM, HASH_WIDTH, IS_BRANCH_C16_POS, IS_BRANCH_C1_POS,
        RLP_NUM,
    },
    mpt_circuit::witness_row::MptWitnessRow,
};

/*
An account leaf occupies 8 rows.
Contrary as in the branch rows, the `S` and `C` leaves are not positioned parallel to each other.
The rows are the following:
ACCOUNT_LEAF_KEY_S
ACCOUNT_LEAF_KEY_C
ACCOUNT_NON_EXISTING
ACCOUNT_LEAF_NONCE_BALANCE_S
ACCOUNT_LEAF_NONCE_BALANCE_C
ACCOUNT_LEAF_STORAGE_CODEHASH_S
ACCOUNT_LEAF_STORAGE_CODEHASH_C
ACCOUNT_DRIFTED_LEAF

The constraints in this file apply to ACCOUNT_NON_EXISTING.

For example, the row might be:
[0,0,0,32,252,237,52,8,133,130,180,167,143,97,28,115,102,25,94,62,148,249,8,6,55,244,16,75,187,208,208,127,251,120,61,73,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]

We are proving that there is no account at the specified address. There are two versions of proof:
    1. A leaf is returned by getProof that is not at the required address (we call this a wrong leaf).
    In this case, the `ACCOUNT_NON_EXISTING` row contains the nibbles of the address (the nibbles that remain
    after the nibbles used for traversing through the branches are removed) that was enquired
    while `ACCOUNT_LEAF_KEY` row contains the nibbles of the wrong leaf. We need to prove that
    the difference is nonzero. This way we prove that there exists some account which has some
    number of the starting nibbles the same as the enquired address (the path through branches
    above the leaf), but at the same time the full address is not the same - the nibbles stored in a leaf differ.
    2. A branch is the last element of the getProof response and there is a nil object
    at the address position. Placeholder account leaf is added in this case.
    In this case, the `ACCOUNT_NON_EXISTING` row contains the same nibbles as `ACCOUNT_LEAF_KEY` and it is
    not needed. We just need to prove that the branch contains nil object (128) at the enquired address.

The whole account leaf looks like:
[248,106,161,32,252,237,52,8,133,130,180,167,143,97,28,115,102,25,94,62,148,249,8,6,55,244,16,75,187,208,208,127,251,120,61,73,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]
[248,106,161,32,252,237,52,8,133,130,180,167,143,97,28,115,102,25,94,62,148,249,8,6,55,244,16,75,187,208,208,127,251,120,61,73,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]
[0,0,0,32,252,237,52,8,133,130,180,167,143,97,28,115,102,25,94,62,148,249,8,6,55,244,16,75,187,208,208,127,251,120,61,73,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]
[184,70,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,68,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]
[184,70,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,68,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]
[0,160,86,232,31,23,27,204,85,166,255,131,69,230,146,192,248,110,91,72,224,27,153,108,173,192,1,98,47,181,227,99,180,33,0,160,197,210,70,1,134,247,35,60,146,126,125,178,220,199,3,192,229,0,182,83,202,130,39,59,123,250,216,4,93,133,164,122]
[0,160,86,232,31,23,27,204,85,166,255,131,69,230,146,192,248,110,91,72,224,27,153,108,173,192,1,98,47,181,227,99,180,33,0,160,197,210,70,1,134,247,35,60,146,126,125,178,220,199,3,192,229,0,182,83,202,130,39,59,123,250,216,4,93,133,164,122]
[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]

We can observe that the example account leaf above is not for non-existing account proof as the first and third
rows contain the same nibbles (the difference is solely in RLP specific bytes which are not needed
in `ACCOUNT_NON_EXISTING` row).

For the example of non-existing account proof account leaf see below:

[248 102 157 55 236 125 29 155 142 209 241 75 145 144 143 254 65 81 209 56 13 192 157 236 195 213 73 132 11 251 149 241 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 6]
[248 102 157 55 236 125 29 155 142 209 241 75 145 144 143 254 65 81 209 56 13 192 157 236 195 213 73 132 11 251 149 241 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 4]
[1 0 157 56 133 130 180 167 143 97 28 115 102 25 94 62 148 249 8 6 55 244 16 75 187 208 208 127 251 120 61 73 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 18]
[184 70 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 248 68 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 7]
[184 70 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 248 68 128 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 8]
[0 160 112 158 181 221 162 20 124 79 184 25 162 13 167 162 146 25 237 242 59 120 184 154 118 137 92 181 187 152 115 82 223 48 0 160 7 190 1 231 231 32 111 227 30 206 233 26 215 93 173 166 90 214 186 67 58 230 71 161 185 51 4 105 247 198 103 124 0 9]
[0 160 112 158 181 221 162 20 124 79 184 25 162 13 167 162 146 25 237 242 59 120 184 154 118 137 92 181 187 152 115 82 223 48 0 160 7 190 1 231 231 32 111 227 30 206 233 26 215 93 173 166 90 214 186 67 58 230 71 161 185 51 4 105 247 198 103 124 0 11]
[0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 10]

In this case, the nibbles in the third row are different from the nibbles in the first or second row. Here, we are
proving that the account does not exist at the address which starts with the same nibbles as the leaf that is
in the rows above (except for the `ACCOUNT_NON_EXISTING` row) and continues with nibbles `ACCOUNT_NON_EXISTING` row.

Note that the selector (being 1 in this case) at `s_main.rlp1` specifies whether it is wrong leaf or nil case.

Lookups:
The `is_non_existing_account_proof` lookup is enabled in `ACCOUNT_NON_EXISTING` row.
*/

#[derive(Clone, Debug)]
pub(crate) struct AccountNonExistingConfig<F> {
    _marker: PhantomData<F>,
}

impl<F: FieldExt> AccountNonExistingConfig<F> {
    #[allow(clippy::too_many_arguments)]
    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        q_enable: impl Fn(&mut VirtualCells<'_, F>) -> Expression<F> + Copy,
        not_first_level: Column<Advice>,
        s_main: MainCols<F>,
        c_main: MainCols<F>,
        accs: AccumulatorCols<F>,
        sel1: Column<Advice>, /* should be the same as sel2 as both parallel proofs are the same
                               * for non_existing_account_proof */
        power_of_randomness: [Expression<F>; HASH_WIDTH],
        fixed_table: [Column<Fixed>; 3],
        address_rlc: Column<Advice>,
    ) -> Self {
        let config = AccountNonExistingConfig {
            _marker: PhantomData,
        };
        let one = Expression::Constant(F::one());
        let c32 = Expression::Constant(F::from(32));
        // key rlc is in the first branch node
        let rot_into_first_branch_child = -(ACCOUNT_NON_EXISTING_IND - 1 + BRANCH_ROWS_NUM);

        let add_wrong_leaf_constraints =
            |meta: &mut VirtualCells<F>,
             constraints: &mut Vec<(&str, Expression<F>)>,
             q_enable: Expression<F>,
             c_rlp1_cur: Expression<F>,
             c_rlp2_cur: Expression<F>,
             correct_level: Expression<F>,
             is_wrong_leaf: Expression<F>| {
                let sum = meta.query_advice(accs.key.rlc, Rotation::cur());
                let sum_prev = meta.query_advice(accs.key.mult, Rotation::cur());
                let diff_inv = meta.query_advice(accs.acc_s.rlc, Rotation::cur());

                let c_rlp1_prev = meta.query_advice(c_main.rlp1, Rotation::prev());
                let c_rlp2_prev = meta.query_advice(c_main.rlp2, Rotation::prev());

                let mut sum_check = Expression::Constant(F::zero());
                let mut sum_prev_check = Expression::Constant(F::zero());
                let mut mult = power_of_randomness[0].clone();
                for ind in 1..HASH_WIDTH {
                    sum_check = sum_check
                        + meta.query_advice(s_main.bytes[ind], Rotation::cur()) * mult.clone();
                    sum_prev_check = sum_prev_check
                        + meta.query_advice(s_main.bytes[ind], Rotation::prev()) * mult.clone();
                    mult = mult * power_of_randomness[0].clone();
                }
                sum_check = sum_check + c_rlp1_cur * mult.clone();
                sum_prev_check = sum_prev_check + c_rlp1_prev * mult.clone();
                mult = mult * power_of_randomness[0].clone();
                sum_check = sum_check + c_rlp2_cur * mult.clone();
                sum_prev_check = sum_prev_check + c_rlp2_prev * mult;

                /*
                We compute the RLC of the key bytes in the ACCOUNT_NON_EXISTING row. We check whether the computed
                value is the same as the one stored in `accs.key.mult` column.
                */
                constraints.push((
                    "Wrong leaf sum check",
                    q_enable.clone()
                        * correct_level.clone()
                        * is_wrong_leaf.clone()
                        * (sum.clone() - sum_check),
                ));

                /*
                We compute the RLC of the key bytes in the ACCOUNT_LEAF_KEY row. We check whether the computed
                value is the same as the one stored in `accs.key.rlc` column.
                */
                constraints.push((
                    "Wrong leaf sum_prev check",
                    q_enable.clone()
                        * correct_level.clone()
                        * is_wrong_leaf.clone()
                        * (sum_prev.clone() - sum_prev_check),
                ));

                /*
                The address in the ACCOUNT_LEAF_KEY row and the address in the ACCOUNT_NON_EXISTING row
                are indeed different.
                */
                constraints.push((
                    "Address of a leaf is different than address being inquired (corresponding to address_rlc)",
                    q_enable
                        * correct_level
                        * is_wrong_leaf
                        * (one.clone() - (sum - sum_prev) * diff_inv),
                ));
            };

        /*
        Checks that account_non_existing_row contains the nibbles that give address_rlc (after considering
        modified_node in branches/extension nodes above).
        Note: currently, for non_existing_account proof S and C proofs are the same, thus there is never
        a placeholder branch.
        */
        meta.create_gate(
            "Non existing account proof leaf address RLC (leaf not in first level, branch not placeholder)",
            |meta| {
                let q_enable = q_enable(meta);
                let mut constraints = vec![];

                let is_leaf_in_first_level =
                    one.clone() - meta.query_advice(not_first_level, Rotation::cur());

                // Wrong leaf has a meaning only for non existing account proof. For this proof, there are two cases:
                // 1. A leaf is returned that is not at the required address (wrong leaf).
                // 2. A branch is returned as the last element of getProof and there is nil object at address position. Placeholder account leaf is added in this case.
                let is_wrong_leaf = meta.query_advice(s_main.rlp1, Rotation::cur());
                // is_wrong_leaf is checked to be bool in account_leaf_nonce_balance (q_enable in this chip
                // is true only when non_existing_account).

                let key_rlc_acc_start =
                    meta.query_advice(accs.key.rlc, Rotation(rot_into_first_branch_child));
                let key_mult_start =
                    meta.query_advice(accs.key.mult, Rotation(rot_into_first_branch_child));

                // sel1, sel2 is in init branch
                let c16 = meta.query_advice(
                    s_main.bytes[IS_BRANCH_C16_POS - RLP_NUM],
                    Rotation(rot_into_first_branch_child - 1),
                );
                let c1 = meta.query_advice(
                    s_main.bytes[IS_BRANCH_C1_POS - RLP_NUM],
                    Rotation(rot_into_first_branch_child - 1),
                );

                let c48 = Expression::Constant(F::from(48));

                // If c16 = 1, we have nibble+48 in s_main.bytes[0].
                let s_advice1 = meta.query_advice(s_main.bytes[1], Rotation::cur());
                let mut key_rlc_acc = key_rlc_acc_start
                    + (s_advice1.clone() - c48) * key_mult_start.clone() * c16.clone();
                let mut key_mult = key_mult_start.clone() * power_of_randomness[0].clone() * c16;
                key_mult = key_mult + key_mult_start * c1.clone(); // set to key_mult_start if sel2, stays key_mult if sel1

                /*
                If there is an even number of nibbles stored in a leaf, `s_advice1` needs to be 32.
                */
                constraints.push((
                    "Account leaf key acc s_advice1",
                    q_enable.clone()
                        * (one.clone() - is_leaf_in_first_level.clone())
                        * is_wrong_leaf.clone()
                        * (s_advice1 - c32.clone())
                        * c1,
                ));

                let s_advices2 = meta.query_advice(s_main.bytes[2], Rotation::cur());
                key_rlc_acc = key_rlc_acc + s_advices2 * key_mult.clone();

                for ind in 3..HASH_WIDTH {
                    let s = meta.query_advice(s_main.bytes[ind], Rotation::cur());
                    key_rlc_acc = key_rlc_acc + s * key_mult.clone() * power_of_randomness[ind - 3].clone();
                }

                let c_rlp1_cur = meta.query_advice(c_main.rlp1, Rotation::cur());
                let c_rlp2_cur = meta.query_advice(c_main.rlp2, Rotation::cur());
                key_rlc_acc = key_rlc_acc + c_rlp1_cur.clone() * key_mult.clone() * power_of_randomness[29].clone();
                key_rlc_acc = key_rlc_acc + c_rlp2_cur.clone() * key_mult * power_of_randomness[30].clone();

                let address_rlc = meta.query_advice(address_rlc, Rotation::cur());

                /*
                Differently as for the other proofs, the account-non-existing proof compares `address_rlc`
                with the address stored in `ACCOUNT_NON_EXISTING` row, not in `ACCOUNT_LEAF_KEY` row.

                The crucial thing is that we have a wrong leaf at the address (not exactly the same, just some starting
                set of nibbles is the same) where we are proving there is no account.
                If there would be an account at the specified address, it would be positioned in the branch where
                the wrong account is positioned. Note that the position is determined by the starting set of nibbles.
                Once we add the remaining nibbles to the starting ones, we need to obtain the enquired address.
                There is a complementary constraint which makes sure the remaining nibbles are different for wrong leaf
                and the non-existing account (in the case of wrong leaf, while the case with nil being in branch
                is different).
                */
                constraints.push((
                    "Account address RLC",
                    q_enable.clone()
                        * (one.clone() - is_leaf_in_first_level.clone())
                        * is_wrong_leaf.clone()
                        * (key_rlc_acc - address_rlc),
                ));

                add_wrong_leaf_constraints(meta, &mut constraints, q_enable.clone(), c_rlp1_cur,
                    c_rlp2_cur, one.clone() - is_leaf_in_first_level.clone(), is_wrong_leaf.clone());
 
                let is_nil_object = meta.query_advice(sel1, Rotation(rot_into_first_branch_child));

                /*
                In case when there is no wrong leaf, we need to check there is a nil object in the parent branch.
                Note that the constraints in `branch.rs` ensure that `sel1` is 1 if and only if there is a nil object
                at `modified_node` position. We check that in case of no wrong leaf in
                the non-existing-account proof, `sel1` is 1.
                */
                constraints.push((
                    "Nil object in parent branch",
                    q_enable
                        * (one.clone() - is_leaf_in_first_level)
                        * (one.clone() - is_wrong_leaf)
                        * (one.clone() - is_nil_object),
                ));

                constraints
            },
        );

        /*
        Ensuring that the account does not exist when there is only one account in the state trie.
        Note 1: The hash of the only account is checked to be the state root in account_leaf_storage_codehash.rs.
        Note 2: There is no nil_object case checked in this gate, because it is covered in the gate
        above. That is because when there is a branch (with nil object) in the first level,
        it automatically means the account leaf is not in the first level.
        */
        meta.create_gate(
            "Non existing account proof leaf address RLC (leaf in first level)",
            |meta| {
                let q_enable = q_enable(meta);
                let mut constraints = vec![];

                let is_leaf_in_first_level =
                    one.clone() - meta.query_advice(not_first_level, Rotation::cur());

                let is_wrong_leaf = meta.query_advice(s_main.rlp1, Rotation::cur());

                // Note: when leaf is in the first level, the key stored in the leaf is always
                // of length 33 - the first byte being 32 (when after branch,
                // the information whether there the key is odd or even
                // is in s_main.bytes[IS_BRANCH_C16_POS - LAYOUT_OFFSET] (see sel1/sel2).

                let s_advice1 = meta.query_advice(s_main.bytes[1], Rotation::cur());
                let mut key_rlc_acc = Expression::Constant(F::zero());

                constraints.push((
                    "Account leaf key acc s_advice1",
                    q_enable.clone()
                        * (s_advice1 - c32)
                        * is_wrong_leaf.clone()
                        * is_leaf_in_first_level.clone(),
                ));

                let s_advices2 = meta.query_advice(s_main.bytes[2], Rotation::cur());
                key_rlc_acc = key_rlc_acc + s_advices2;

                for ind in 3..HASH_WIDTH {
                    let s = meta.query_advice(s_main.bytes[ind], Rotation::cur());
                    key_rlc_acc = key_rlc_acc + s * power_of_randomness[ind - 3].clone();
                }

                let c_rlp1_cur = meta.query_advice(c_main.rlp1, Rotation::cur());
                let c_rlp2_cur = meta.query_advice(c_main.rlp2, Rotation::cur());
                key_rlc_acc = key_rlc_acc + c_rlp1_cur.clone() * power_of_randomness[29].clone();
                key_rlc_acc = key_rlc_acc + c_rlp2_cur.clone() * power_of_randomness[30].clone();

                let address_rlc = meta.query_advice(address_rlc, Rotation::cur());

                constraints.push((
                    "Computed account address RLC same as value in address_rlc column",
                    q_enable.clone()
                        * is_leaf_in_first_level.clone()
                        * is_wrong_leaf.clone()
                        * (key_rlc_acc - address_rlc),
                ));

                add_wrong_leaf_constraints(
                    meta,
                    &mut constraints,
                    q_enable,
                    c_rlp1_cur,
                    c_rlp2_cur,
                    is_leaf_in_first_level,
                    is_wrong_leaf,
                );

                constraints
            },
        );

        meta.create_gate(
            "Address of wrong leaf and the enquired address are of the same length",
            |meta| {
                let q_enable = q_enable(meta);
                let mut constraints = vec![];

                let is_wrong_leaf = meta.query_advice(s_main.rlp1, Rotation::cur());
                let s_advice0_prev = meta.query_advice(s_main.bytes[0], Rotation::prev());
                let s_advice0_cur = meta.query_advice(s_main.bytes[0], Rotation::cur());

                /*
                This constraint is to prevent the attacker to prove that some account does not exist by setting
                some arbitrary number of nibbles in the account leaf which would lead to a desired RLC.
                */
                constraints.push((
                    "The number of nibbles in the wrong leaf and the enquired address are the same",
                    q_enable * is_wrong_leaf * (s_advice0_cur - s_advice0_prev),
                ));

                constraints
            },
        );

        /*
        /*
        Key RLC is computed over all of `s_main.bytes[1], ..., s_main.bytes[31], c_main.rlp1, c_main.rlp2`
        because we do not know the key length in advance.
        To prevent changing the key and setting `s_main.bytes[i]` (or `c_main.rlp1/c_main.rlp2`) for
        `i > key_len + 1` to get the desired key RLC, we need to ensure that
        `s_main.bytes[i] = 0` for `i > key_len + 1`.

        Note that the number of the key bytes in the `ACCOUNT_NON_EXISTING` row needs to be the same as
        the number of the key bytes in the `ACCOUNT_LEAF_KEY` row.

        Note: the key length is always in s_main.bytes[0] here as opposed to storage
        key leaf where it can appear in `s_rlp2` too. This is because the account
        leaf contains nonce, balance, ... which makes it always longer than 55 bytes,
        which makes a RLP to start with 248 (`s_rlp1`) and having one byte (in `s_rlp2`)
        for the length of the remaining stream.
        */
        for ind in 1..HASH_WIDTH {
            key_len_lookup(
                meta,
                q_enable,
                ind,
                s_main.bytes[0],
                s_main.bytes[ind],
                128,
                fixed_table,
            )
        }
        key_len_lookup(meta, q_enable, 32, s_main.bytes[0], c_main.rlp1, 128, fixed_table);
        key_len_lookup(meta, q_enable, 33, s_main.bytes[0], c_main.rlp2, 128, fixed_table);
        */

        /*
        Range lookups ensure that `s_main`, `c_main.rlp1`, `c_main.rlp2` columns are all bytes (between 0 - 255).
        Note that `c_main.bytes` columns are not used.
        */
        range_lookups(
            meta,
            q_enable,
            s_main.bytes.to_vec(),
            FixedTableTag::Range256,
            fixed_table,
        );
        range_lookups(
            meta,
            q_enable,
            [s_main.rlp2, c_main.rlp1, c_main.rlp2].to_vec(),
            FixedTableTag::Range256,
            fixed_table,
        );

        config
    }

    pub fn assign(
        &self,
        region: &mut Region<'_, F>,
        mpt_config: &MPTConfig<F>,
        witness: &[MptWitnessRow<F>],
        offset: usize,
    ) {
        let row_prev = &witness[offset - 1];
        let row = &witness[offset];
        let key_len = row_prev.get_byte(2) as usize - 128;
        let mut sum = F::zero();
        let mut sum_prev = F::zero();
        let mut mult = mpt_config.randomness;
        for i in 0..key_len {
            sum += F::from(row.get_byte(3 + i) as u64) * mult;
            sum_prev += F::from(row_prev.get_byte(3 + i) as u64) * mult;
            mult *= mpt_config.randomness;
        }
        let mut diff_inv = F::zero();
        if sum != sum_prev {
            diff_inv = F::invert(&(sum - sum_prev)).unwrap();
        }

        region
            .assign_advice(
                || "assign sum".to_string(),
                mpt_config.accumulators.key.rlc,
                offset,
                || Value::known(sum),
            )
            .ok();
        region
            .assign_advice(
                || "assign sum prev".to_string(),
                mpt_config.accumulators.key.mult,
                offset,
                || Value::known(sum_prev),
            )
            .ok();
        region
            .assign_advice(
                || "assign diff inv".to_string(),
                mpt_config.accumulators.acc_s.rlc,
                offset,
                || Value::known(diff_inv),
            )
            .ok();

        if row.get_byte_rev(IS_NON_EXISTING_ACCOUNT_POS) == 1 {
            region
                .assign_advice(
                    || "assign lookup enabled".to_string(),
                    mpt_config.proof_type.proof_type,
                    offset,
                    || Value::known(F::from(4_u64)), // non existing account lookup enabled in this row if it is non_existing_account proof
                )
                .ok();
        }
    }
}

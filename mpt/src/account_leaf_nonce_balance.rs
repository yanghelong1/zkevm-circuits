use halo2_proofs::{
    circuit::Chip,
    plonk::{Advice, Column, ConstraintSystem, Expression, Fixed, VirtualCells},
    poly::Rotation,
};
use itertools::Itertools;
use pairing::arithmetic::FieldExt;
use std::marker::PhantomData;

use crate::{
    helpers::{compute_rlc, get_bool_constraint, key_len_lookup, mult_diff_lookup, range_lookups},
    mpt::{FixedTableTag, MainCols, ProofTypeCols},
    param::{
        ACCOUNT_LEAF_KEY_C_IND, ACCOUNT_LEAF_KEY_S_IND, ACCOUNT_LEAF_NONCE_BALANCE_C_IND,
        ACCOUNT_LEAF_NONCE_BALANCE_S_IND, HASH_WIDTH, ACCOUNT_NON_EXISTING_IND,
    },
};

#[derive(Clone, Debug)]
pub(crate) struct AccountLeafNonceBalanceConfig {}

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

This chip applies to ACCOUNT_LEAF_NONCE_BALANCE_S and ACCOUNT_LEAF_NONCE_BALANCE_C rows.

For example, the two rows might be:
[184,70,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,68,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]
[184,70,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,68,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]

In `ACCOUNT_LEAF_NONCE_BALANCE_S` example row, there is `S` nonce stored in `s_main` and `S` balance in
`c_main`. We can see nonce in `S` proof is `0 = 128 - 128`.

In `ACCOUNT_LEAF_NONCE_BALANCE_C` example row, there is `C` nonce stored in `s_main` and `C` balance in
`c_main`. We can see nonce in `C` proof is `1`.


*/
pub(crate) struct AccountLeafNonceBalanceChip<F> {
    config: AccountLeafNonceBalanceConfig,
    _marker: PhantomData<F>,
}

impl<F: FieldExt> AccountLeafNonceBalanceChip<F> {
    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        proof_type: ProofTypeCols,
        q_enable: impl Fn(&mut VirtualCells<'_, F>) -> Expression<F> + Copy,
        s_main: MainCols,
        c_main: MainCols,
        acc_s: Column<Advice>,
        acc_mult_s: Column<Advice>,
        acc_mult_c: Column<Advice>,
        mult_diff_nonce: Column<Advice>,
        mult_diff_balance: Column<Advice>,
        r_table: Vec<Expression<F>>,
        s_mod_node_hash_rlc: Column<Advice>,
        c_mod_node_hash_rlc: Column<Advice>,
        sel1: Column<Advice>,
        sel2: Column<Advice>,
        fixed_table: [Column<Fixed>; 3],
        is_s: bool,
    ) -> AccountLeafNonceBalanceConfig {
        let config = AccountLeafNonceBalanceConfig {};
        let one = Expression::Constant(F::one());
        let c128 = Expression::Constant(F::from(128));

        meta.create_gate("Account leaf nonce balance RLC & RLP", |meta| {
            let q_enable = q_enable(meta);
            let mut constraints = vec![];

            /*
            [248,112,157,59,158,160,175,159,65,212,107,23,98,208,38,205,150,63,244,2,185,236,246,95,240,224,191,229,27,102,202,231,184,80
            There are 112 bytes after the first two bytes.
            157 means the key is 29 (157 - 128) bytes long.
            */

            // Nonce, balance, storage, codehash are string in RLP: s_rlp1 and s_rlp2
            // contains the length of this string, for example 184 80 means the second
            // part is of length 1 (183 + 1 = 184) and there are 80 bytes in this string.
            // Then there is a list rlp meta data 248 78 where (this is stored in c_rlp1 and
            // c_rlp2) 78 = 3 (nonce) + 9 (balance) + 33 (storage) + 33
            // (codehash). We have nonce in s_main.bytes and balance in c_main.bytes.
            // s_rlp1  s_rlp2  c_rlp1  c_rlp2  s_main.bytes  c_main.bytes
            // 184     80      248     78      nonce      balance

            let mut rot = -(ACCOUNT_LEAF_NONCE_BALANCE_S_IND - ACCOUNT_LEAF_KEY_S_IND);
            let mut rot_into_non_existing = -(ACCOUNT_LEAF_NONCE_BALANCE_S_IND - ACCOUNT_NON_EXISTING_IND);
            if !is_s {
                rot = -(ACCOUNT_LEAF_NONCE_BALANCE_C_IND - ACCOUNT_LEAF_KEY_C_IND);
                rot_into_non_existing = -(ACCOUNT_LEAF_NONCE_BALANCE_C_IND - ACCOUNT_NON_EXISTING_IND);
            }
            let c248 = Expression::Constant(F::from(248));

            let acc_prev = meta.query_advice(acc_s, Rotation(rot));
            let acc_mult_prev = meta.query_advice(acc_mult_s, Rotation(rot));

            let acc_mult_after_nonce = meta.query_advice(acc_mult_c, Rotation::cur());
            let mult_diff_nonce = meta.query_advice(mult_diff_nonce, Rotation::cur());
            let mult_diff_balance = meta.query_advice(mult_diff_balance, Rotation::cur());

            let is_nonce_long = meta.query_advice(
                sel1,
                Rotation(rot),
            );
            let is_balance_long = meta.query_advice(
                sel2,
                Rotation(rot),
            );

            // Nonce and balance can occupy 1 or more bytes. If 1 byte, we say it is short. If more than 1 byte,
            // we say it is long.
            constraints.push((
                "Bool check is_nonce_long",
                get_bool_constraint(q_enable.clone(), is_nonce_long.clone()),
            ));
            constraints.push((
                "Bool check is_balance_long",
                get_bool_constraint(q_enable.clone(), is_balance_long.clone()),
            ));

            for ind in 1..HASH_WIDTH {
                let s = meta.query_advice(s_main.bytes[ind], Rotation::cur());
                let c = meta.query_advice(c_main.bytes[ind], Rotation::cur());
                constraints.push((
                    "s_main.bytes[i] = 0 for i > 0 when is_nonce_short",
                    q_enable.clone() * (one.clone() - is_nonce_long.clone()) * s.clone(),
                ));
                constraints.push((
                    "c_main.bytes[i] = 0 for i > 0 when is_balance_short",
                    q_enable.clone() * (one.clone() - is_balance_long.clone()) * c.clone(),
                ));
            }

            let key_len = meta.query_advice(s_main.bytes[0], Rotation(rot)) - c128.clone();
            let s_advices0_cur = meta.query_advice(s_main.bytes[0], Rotation::cur());
            let s_advices1_cur = meta.query_advice(s_main.bytes[1], Rotation::cur());

            // When non_existing_account_proof and wrong leaf, these constraints need to be checked (the wrong
            // leaf is being checked). When non_existing_account_proof and not wrong leaf (there are only branches
            // in the proof and a placeholder account leaf), these constraints are not checked. It is checked
            // that there is nil in the parent branch at the proper position (see account_non_existing), note
            // that we need (placeholder) account leaf for lookups and to know when to check that parent branch
            // has a nil.
            let is_wrong_leaf = meta.query_advice(s_main.rlp1, Rotation(rot_into_non_existing));
            let is_non_existing_account_proof = meta.query_advice(proof_type.is_non_existing_account_proof, Rotation::cur());

            constraints.push((
                "is_wrong_leaf is bool",
                q_enable.clone()
                    * (one.clone() - is_wrong_leaf.clone())
                    * is_wrong_leaf.clone(),
            ));
            // Note: (is_non_existing_account_proof.clone() - is_wrong_leaf.clone() - one.clone())
            // cannot be 0 when is_non_existing_account_proof = 0.

            let s_rlp1 = meta.query_advice(s_main.rlp1, Rotation::cur());
            let rlp_len = meta.query_advice(s_main.rlp2, Rotation(rot));
            let s_rlp2 = meta.query_advice(s_main.rlp2, Rotation::cur());

            let mut expr = acc_prev + s_rlp1.clone() * acc_mult_prev.clone();
            let mut rind = 0;
            expr = expr + s_rlp2.clone() * acc_mult_prev.clone() * r_table[rind].clone();
            rind += 1;

            let c_rlp1 = meta.query_advice(c_main.rlp1, Rotation::cur());
            let c_rlp2 = meta.query_advice(c_main.rlp2, Rotation::cur());
            constraints.push((
                "leaf nonce balance c_rlp1",
                q_enable.clone()
                * (is_non_existing_account_proof.clone() - is_wrong_leaf.clone() - one.clone())
                * (c_rlp1.clone() - c248.clone()),
            ));
            expr = expr + c_rlp1.clone() * acc_mult_prev.clone() * r_table[rind].clone();
            rind += 1;
            expr = expr + c_rlp2.clone() * acc_mult_prev.clone() * r_table[rind].clone();
            rind += 1;

            let nonce_value_long_rlc = s_advices1_cur.clone()
                + compute_rlc(
                    meta,
                    s_main.bytes.iter().skip(2).map(|v| *v).collect_vec(),
                    0,
                    one.clone(),
                    0,
                    r_table.clone(),
                );

            let nonce_rlc = (s_advices0_cur.clone() + nonce_value_long_rlc.clone() * r_table[0].clone()) * is_nonce_long.clone()
                + s_advices0_cur.clone() * (one.clone() - is_nonce_long.clone());

            let nonce_stored = meta.query_advice(s_mod_node_hash_rlc, Rotation::cur());
            constraints.push((
                "nonce RLC long",
                q_enable.clone() * is_nonce_long.clone() * (nonce_value_long_rlc.clone() - nonce_stored.clone()),
            ));
            constraints.push((
                "nonce RLC short",
                q_enable.clone() * (one.clone() - is_nonce_long.clone()) * (s_advices0_cur.clone() - nonce_stored.clone()),
            ));

            expr = expr + nonce_rlc * r_table[rind].clone() * acc_mult_prev.clone();

            let c_advices0_cur = meta.query_advice(c_main.bytes[0], Rotation::cur());
            let c_advices1_cur = meta.query_advice(c_main.bytes[1], Rotation::cur());
            let balance_stored = meta.query_advice(c_mod_node_hash_rlc, Rotation::cur());
            let balance_value_long_rlc = c_advices1_cur.clone()
                + compute_rlc(
                    meta,
                    c_main.bytes.iter().skip(2).map(|v| *v).collect_vec(),
                    0,
                    one.clone(),
                    0,
                    r_table.clone(),
                );

            let balance_rlc = (c_advices0_cur.clone() + balance_value_long_rlc.clone() * r_table[0].clone()) * is_balance_long.clone()
                + c_advices0_cur.clone() * (one.clone() - is_balance_long.clone());

            constraints.push((
                "balance RLC long",
                q_enable.clone() * is_balance_long.clone() * (balance_value_long_rlc.clone() - balance_stored.clone()),
            ));
            constraints.push((
                "balance RLC short",
                q_enable.clone() * (one.clone() - is_balance_long.clone()) * (c_advices0_cur.clone() - balance_stored.clone()),
            ));

            if !is_s {
                let nonce_s_from_prev = meta.query_advice(s_mod_node_hash_rlc, Rotation::prev());
                let nonce_s_from_cur = meta.query_advice(sel1, Rotation::cur());
                let balance_s_from_prev = meta.query_advice(c_mod_node_hash_rlc, Rotation::prev());
                let balance_s_from_cur = meta.query_advice(sel2, Rotation::cur());

                // Note: when nonce or balance is 0, the actual value in the RLP encoding is 128!

                // We need correct previous nonce to enable lookup in nonce balance C row:
                constraints.push((
                    "nonce prev RLC",
                    q_enable.clone() * (nonce_s_from_prev.clone() - nonce_s_from_cur.clone()),
                ));
                // We need correct previous balance to enable lookup in nonce balance C row:
                constraints.push((
                    "balance prev RLC",
                    q_enable.clone() * (balance_s_from_prev.clone() - balance_s_from_cur.clone()),
                ));

                // Check there is only one modification at once:
                let is_storage_mod = meta.query_advice(proof_type.is_storage_mod, Rotation::cur());
                let is_nonce_mod = meta.query_advice(proof_type.is_nonce_mod, Rotation::cur());
                let is_balance_mod = meta.query_advice(proof_type.is_balance_mod, Rotation::cur());
                let is_account_delete_mod = meta.query_advice(proof_type.is_account_delete_mod, Rotation::cur());

                constraints.push((
                    "if storage / codehash / balance mod: nonce_s = nonce_c",
                    q_enable.clone()
                        * (is_storage_mod.clone()
                            + is_balance_mod.clone())
                        * (one.clone() - is_account_delete_mod.clone())
                        * (nonce_s_from_cur.clone() - nonce_stored.clone()),
                ));
                constraints.push((
                    "if storage / codehash / nonce mod: balance_s = balance_c",
                    q_enable.clone()
                        * (is_storage_mod.clone() + is_nonce_mod.clone())
                        * (one.clone() - is_account_delete_mod.clone())
                        * (balance_s_from_cur.clone() - balance_stored.clone()),
                ));
            }

            expr = expr + balance_rlc * acc_mult_after_nonce.clone();

            let acc = meta.query_advice(acc_s, Rotation::cur());
            constraints.push(("leaf nonce balance acc", q_enable.clone() * (expr - acc)));

            let acc_mult_final = meta.query_advice(acc_mult_s, Rotation::cur());

            constraints.push((
                "leaf nonce acc mult (nonce long)",
                q_enable.clone()
                    * is_nonce_long.clone()
                    * (acc_mult_after_nonce.clone()
                        - acc_mult_prev.clone() * mult_diff_nonce.clone()),
            ));
            constraints.push((
                "leaf nonce acc mult (nonce short)",
                q_enable.clone()
                    * (one.clone() - is_nonce_long.clone())
                    * (acc_mult_after_nonce.clone() - acc_mult_prev.clone() * r_table[4].clone()), // r_table[4] because of s_rlp1, s_rlp2, c_rlp1, c_rlp2, and 1 for nonce_len = 1
            ));

            // Balance mult:

            constraints.push((
                "leaf nonce acc mult",
                q_enable.clone()
                    * (acc_mult_final.clone()
                        - acc_mult_after_nonce.clone() * mult_diff_balance.clone()),
            ));

            // RLP:
            let c66 = Expression::Constant(F::from(66)); // the number of bytes in storage codehash row
            let c184 = Expression::Constant(F::from(184));
            let nonce_long_len = s_advices0_cur - c128.clone() + one.clone();

            let nonce_len =
                nonce_long_len * is_nonce_long.clone() + (one.clone() - is_nonce_long.clone());

            let balance_long_len = c_advices0_cur - c128.clone() + one.clone();
            let balance_len = balance_long_len * is_balance_long.clone()
                + (one.clone() - is_balance_long.clone());

            /*
            s_rlp1  s_rlp2  c_rlp1  c_rlp2  s_main.bytes  c_main.bytes
            184     80      248     78      nonce      balance

            Or:
            s_rlp1  s_rlp2  c_rlp1  c_rlp2  s_main.bytes                         c_main.bytes
            248     109     157     (this is key row, 157 means key of length 29)
            184     77      248     75      7 (short nonce , only one byte)   135 (means balance is of length 7) 28 ... 59
            */

            constraints.push(("RLP 1", q_enable.clone()
                * (is_non_existing_account_proof.clone() - is_wrong_leaf.clone() - one.clone())
                * (s_rlp1.clone() - c184)));
            constraints.push(("RLP 2", q_enable.clone()
                * (is_non_existing_account_proof.clone() - is_wrong_leaf.clone() - one.clone())
                * (c_rlp1.clone() - c248)));
            constraints.push((
                "RLP 3",
                q_enable.clone()
                * (is_non_existing_account_proof.clone() - is_wrong_leaf.clone() - one.clone())
                * (s_rlp2.clone() - c_rlp2.clone() - one.clone() - one.clone()),
            ));
            constraints.push((
                "RLP 4",
                q_enable.clone()
                * (is_non_existing_account_proof.clone() - is_wrong_leaf.clone() - one.clone())
                * (c_rlp2.clone() - nonce_len - balance_len - c66),
            ));
            constraints.push((
                "account leaf RLP length",
                q_enable.clone()
                    * (is_non_existing_account_proof.clone() - is_wrong_leaf.clone() - one.clone())
                    * (rlp_len - key_len - one.clone() - s_rlp2 - one.clone() - one.clone()),
                // -1 because key_len is stored in 1 column
                // -1 because of s_rlp1
                // -1 because of s_rlp2
            ));

            constraints
        });

        let q_enable_nonce_long = |meta: &mut VirtualCells<F>| {
            let q_enable = q_enable(meta);
            let mut is_nonce_long = meta.query_advice(
                sel1,
                Rotation(-(ACCOUNT_LEAF_NONCE_BALANCE_S_IND - ACCOUNT_LEAF_KEY_S_IND)),
            );
            if !is_s {
                is_nonce_long = meta.query_advice(
                    sel1,
                    Rotation(-(ACCOUNT_LEAF_NONCE_BALANCE_C_IND - ACCOUNT_LEAF_KEY_C_IND)),
                );
            }

            q_enable * is_nonce_long
        };
        // mult_diff_nonce corresponds to nonce length:
        mult_diff_lookup(
            meta,
            q_enable_nonce_long, /* mult_diff is acc_r when is_nonce_short (mult_diff doesn't
                                  * need to be checked as it's not used) */
            5, /* 4 for s_rlp1, s_rlp2, c_rlp1, c_rlp1; 1 for byte with length
                * info */
            s_main.bytes[0],
            mult_diff_nonce,
            128,
            fixed_table,
        );

        // There are zeros in s_main.bytes after nonce length:
        /*
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
        */

        let q_enable_balance_long = |meta: &mut VirtualCells<F>| {
            let q_enable = q_enable(meta);
            let mut is_balance_long = meta.query_advice(
                sel1,
                Rotation(-(ACCOUNT_LEAF_NONCE_BALANCE_S_IND - ACCOUNT_LEAF_KEY_S_IND)),
            );
            if !is_s {
                is_balance_long = meta.query_advice(
                    sel1,
                    Rotation(-(ACCOUNT_LEAF_NONCE_BALANCE_C_IND - ACCOUNT_LEAF_KEY_C_IND)),
                );
            }

            q_enable * is_balance_long
        };
        // mult_diff_balance corresponds to balance length:
        mult_diff_lookup(
            meta,
            q_enable_balance_long, /* mult_diff is acc_r when is_balance_short (mult_diff
                                    * doesn't need to be
                                    * checked as it's not used) */
            1, // 1 for byte with length info
            c_main.bytes[0],
            mult_diff_balance,
            128,
            fixed_table,
        );

        // There are zeros in c_main.bytes after balance length:
        /*
        for ind in 1..HASH_WIDTH {
            key_len_lookup(
                meta,
                q_enable,
                ind,
                c_main.bytes[0],
                c_main.bytes[ind],
                128,
                fixed_table,
            )
        }
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
            c_main.bytes.to_vec(),
            FixedTableTag::Range256,
            fixed_table,
        );
        // c_rlp1 is always 248 (checked above)
        range_lookups(
            meta,
            q_enable,
            [s_main.rlp1, s_main.rlp2, c_main.rlp2].to_vec(),
            FixedTableTag::Range256,
            fixed_table,
        );

        config
    }

    pub fn construct(config: AccountLeafNonceBalanceConfig) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }
}

impl<F: FieldExt> Chip<F> for AccountLeafNonceBalanceChip<F> {
    type Config = AccountLeafNonceBalanceConfig;
    type Loaded = ();

    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}

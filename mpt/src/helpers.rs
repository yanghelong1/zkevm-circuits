use halo2_proofs::{
    plonk::{Advice, Column, ConstraintSystem, Expression, Fixed, VirtualCells},
    poly::Rotation,
};
use pairing::arithmetic::FieldExt;

use crate::{
    mpt::FixedTableTag,
    param::{
        HASH_WIDTH, IS_EXT_LONG_EVEN_C16_POS, IS_EXT_LONG_EVEN_C1_POS, IS_EXT_LONG_ODD_C16_POS,
        IS_EXT_LONG_ODD_C1_POS, IS_EXT_SHORT_C16_POS, IS_EXT_SHORT_C1_POS, RLP_NUM,
        R_TABLE_LEN,
    },
};

// Turn 32 hash cells into 4 cells containing keccak words.
pub fn into_words_expr<F: FieldExt>(hash: Vec<Expression<F>>) -> Vec<Expression<F>> {
    let mut words = vec![];
    for i in 0..4 {
        let mut word = Expression::Constant(F::zero());
        let mut exp = Expression::Constant(F::one());
        for j in 0..8 {
            word = word + hash[i * 8 + j].clone() * exp.clone();
            exp = exp * Expression::Constant(F::from(256));
        }
        words.push(word)
    }

    words
}

pub fn compute_rlc<F: FieldExt>(
    meta: &mut VirtualCells<F>,
    advices: Vec<Column<Advice>>,
    mut rind: usize,
    mult: Expression<F>,
    rotation: i32,
    r_table: Vec<Expression<F>>,
) -> Expression<F> {
    let mut r_wrapped = false;
    let mut rlc = Expression::Constant(F::zero());
    for col in advices.iter() {
        let s = meta.query_advice(*col, Rotation(rotation));
        if !r_wrapped {
            rlc = rlc + s * r_table[rind].clone() * mult.clone();
        } else {
            rlc = rlc + s * r_table[rind].clone() * r_table[R_TABLE_LEN - 1].clone() * mult.clone();
        }
        if rind == R_TABLE_LEN - 1 {
            rind = 0;
            r_wrapped = true;
        } else {
            rind += 1;
        }
    }

    rlc
}

pub fn range_lookups<F: FieldExt>(
    meta: &mut ConstraintSystem<F>,
    q_enable: impl Fn(&mut VirtualCells<'_, F>) -> Expression<F>,
    columns: Vec<Column<Advice>>,
    tag: FixedTableTag,
    fixed_table: [Column<Fixed>; 3],
) {
    for col in columns {
        meta.lookup_any("range_lookup", |meta| {
            let q_enable = q_enable(meta);
            let mut constraints = vec![];

            let s = meta.query_advice(col, Rotation::cur());
            constraints.push((
                Expression::Constant(F::from(tag.clone() as u64)),
                meta.query_fixed(fixed_table[0], Rotation::cur()),
            ));
            constraints.push((
                q_enable.clone() * s,
                meta.query_fixed(fixed_table[1], Rotation::cur()),
            ));

            constraints
        });
    }
}

// The columns after the key stops have to be 0 to prevent attacks on RLC using
// bytes that should be 0.
// Let's say we have a key of length 3, then: [248,112,131,59,158,123,0,0,0,...
// 131 - 128 = 3 presents key length. We need to prove all bytes after key ends
// are 0 (after 59, 158, 123).
// We prove the following (33 is max key length):
// (key_len - 1) * 59 < 33 * 255
// (key_len - 2) * 158 < 33 * 255
// (key_len - 3) * 123 < 33 * 255
// From now on, key_len < 0:
// (key_len - 4) * byte < 33 * 255 (Note that this will be true only if byte =
// 0) (key_len - 5) * byte < 33 * 255 (Note that this will be true only if byte
// = 0) (key_len - 6) * byte < 33 * 255 (Note that this will be true only if
// byte = 0) ...
pub fn key_len_lookup<F: FieldExt>(
    meta: &mut ConstraintSystem<F>,
    q_enable: impl Fn(&mut VirtualCells<'_, F>) -> Expression<F>,
    ind: usize,
    key_len_col: Column<Advice>,
    column: Column<Advice>,
    len_offset: usize,
    fixed_table: [Column<Fixed>; 3],
) {
    meta.lookup_any("key_len_lookup", |meta| {
        let mut constraints = vec![];
        let q_enable = q_enable(meta);

        let s = meta.query_advice(column, Rotation::cur());
        let offset = Expression::Constant(F::from(len_offset as u64));
        let key_len = meta.query_advice(key_len_col, Rotation::cur()) - offset;
        let key_len_rem = key_len - Expression::Constant(F::from(ind as u64));
        constraints.push((
            Expression::Constant(F::from(FixedTableTag::RangeKeyLen256 as u64)),
            meta.query_fixed(fixed_table[0], Rotation::cur()),
        ));
        constraints.push((
            q_enable.clone() * s * key_len_rem,
            meta.query_fixed(fixed_table[1], Rotation::cur()),
        ));

        constraints
    });
}

pub fn mult_diff_lookup<F: FieldExt>(
    meta: &mut ConstraintSystem<F>,
    q_enable: impl Fn(&mut VirtualCells<'_, F>) -> Expression<F>,
    addition: usize,
    key_len_col: Column<Advice>,
    mult_diff_col: Column<Advice>,
    key_len_offset: usize,
    fixed_table: [Column<Fixed>; 3],
) {
    meta.lookup_any("mult_diff_lookup", |meta| {
        let q_enable = q_enable(meta);
        let mut constraints = vec![];

        let offset = Expression::Constant(F::from(key_len_offset as u64));
        let key_len = meta.query_advice(key_len_col, Rotation::cur()) - offset;
        let mult_diff = meta.query_advice(mult_diff_col, Rotation::cur());
        let add_expr = Expression::Constant(F::from(addition as u64));

        constraints.push((
            Expression::Constant(F::from(FixedTableTag::RMult as u64)),
            meta.query_fixed(fixed_table[0], Rotation::cur()),
        ));
        constraints.push((
            q_enable.clone() * (key_len + add_expr),
            meta.query_fixed(fixed_table[1], Rotation::cur()),
        ));
        constraints.push((
            q_enable.clone() * mult_diff,
            meta.query_fixed(fixed_table[2], Rotation::cur()),
        ));

        constraints
    });
}

pub fn get_bool_constraint<F: FieldExt>(
    q_enable: Expression<F>,
    expr: Expression<F>,
) -> Expression<F> {
    let one = Expression::Constant(F::from(1_u64));
    q_enable * expr.clone() * (one - expr.clone())
}

pub fn get_is_extension_node<F: FieldExt>(
    meta: &mut VirtualCells<F>,
    s_advices: [Column<Advice>; HASH_WIDTH],
    rot: i32,
) -> Expression<F> {
    // To reduce the expression degree, we pack together multiple information.
    // Constraints on selectors are in extension_node.
    // NOTE: even and odd refers to number of nibbles that are compactly encoded.
    let is_ext_short_c16 = meta.query_advice(
        s_advices[IS_EXT_SHORT_C16_POS - RLP_NUM],
        Rotation(rot),
    );
    let is_ext_short_c1 = meta.query_advice(
        s_advices[IS_EXT_SHORT_C1_POS - RLP_NUM],
        Rotation(rot),
    );
    let is_ext_long_even_c16 = meta.query_advice(
        s_advices[IS_EXT_LONG_EVEN_C16_POS - RLP_NUM],
        Rotation(rot),
    );
    let is_ext_long_even_c1 = meta.query_advice(
        s_advices[IS_EXT_LONG_EVEN_C1_POS - RLP_NUM],
        Rotation(rot),
    );
    let is_ext_long_odd_c16 = meta.query_advice(
        s_advices[IS_EXT_LONG_ODD_C16_POS - RLP_NUM],
        Rotation(rot),
    );
    let is_ext_long_odd_c1 = meta.query_advice(
        s_advices[IS_EXT_LONG_ODD_C1_POS - RLP_NUM],
        Rotation(rot),
    );

    is_ext_short_c16
        + is_ext_short_c1
        + is_ext_long_even_c16
        + is_ext_long_even_c1
        + is_ext_long_odd_c16
        + is_ext_long_odd_c1
}

pub fn get_is_extension_node_one_nibble<F: FieldExt>(
    meta: &mut VirtualCells<F>,
    s_advices: [Column<Advice>; HASH_WIDTH],
    rot: i32,
) -> Expression<F> {
    let is_ext_short_c16 = meta.query_advice(
        s_advices[IS_EXT_SHORT_C16_POS - RLP_NUM],
        Rotation(rot),
    );
    let is_ext_short_c1 = meta.query_advice(
        s_advices[IS_EXT_SHORT_C1_POS - RLP_NUM],
        Rotation(rot),
    );

    is_ext_short_c16 + is_ext_short_c1
}

pub(crate) fn bytes_into_rlc<F: FieldExt>(message: &[u8], r: F) -> F {
    let mut rlc = F::zero();
    let mut mult = F::one();
    for i in 0..message.len() {
        rlc += F::from(message[i] as u64) * mult;
        mult *= r;
    }

    rlc
}

pub(crate) fn bytes_expr_into_rlc<F: FieldExt>(message: &[Expression<F>], r: F) -> Expression<F> {
    let mut rlc = Expression::Constant(F::zero());
    let mut mult = F::one();
    for i in 0..message.len() {
        rlc = rlc + message[i].clone() * mult;
        mult *= r;
    }

    rlc
}

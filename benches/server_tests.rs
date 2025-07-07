use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::Rng;

// Import Prio2 and Prio3 types
#[cfg(feature = "experimental")]
use prio::vdaf::prio2::Prio2;

use prio::vdaf::prio3::{Prio3Count, Prio3SumVec};
use prio::vdaf::{Aggregator, Client, PrepareTransition};

const BATCH_SIZES: [usize; 3] = [1, 100, 10000];
const VEC_LEN: [usize; 3] = [1, 10, 100];
const NUM_AGGREGATORS: usize = 2;
const CONTEXT: &[u8] = b"benchmark context";
const VERIFY_KEY: [u8; 32] = [0u8; 32];

#[derive(Clone, Copy)]
enum ProtocolType {
    Prio2,
    Prio3,
}

#[cfg(feature = "experimental")]
fn verification_bench(c: &mut Criterion, ptype: ProtocolType, group_name: &str, seed: u64) {
    let mut group = c.benchmark_group(group_name);
    group.sample_size(10);
    for &batch_size in &BATCH_SIZES {
        for &vec_len in &VEC_LEN {
            group.bench_with_input(
                BenchmarkId::from_parameter(format!("batch_{}_vec_{}", batch_size, vec_len)), 
                &(batch_size, vec_len), 
                |b, &(batch_size, vec_len)| {
                    let mut rng = StdRng::seed_from_u64(seed);
                    match ptype {
                        ProtocolType::Prio2  => {
                            let vdaf = Prio2::new(vec_len).unwrap();
                            let mut reports = Vec::with_capacity(batch_size);
                            for _ in 0..batch_size {
                                let measurement: Vec<u32> = (0..vec_len).map(|i| (i & 1) as u32).collect();
                                let nonce: [u8; 16] = rng.random();
                                let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &nonce).unwrap();
                                reports.push((public_share, nonce, input_shares));
                            }
                            b.iter(|| {
                                for (public_share, nonce, input_shares) in &reports {
                                    for (agg_id, input_share) in input_shares.iter().enumerate() {
                                        let _ = vdaf.prepare_init(
                                            &VERIFY_KEY,
                                            CONTEXT,
                                            agg_id,
                                            &(),
                                            nonce,
                                            public_share,
                                            input_share,
                                        ).unwrap();
                                    }
                                }
                            });
                        }
                        ProtocolType::Prio3 => {
                            if vec_len == 1 {
                                // Use Prio3Count for single boolean values
                                let vdaf = Prio3Count::new_count(NUM_AGGREGATORS as u8).unwrap();
                                let mut reports = Vec::with_capacity(batch_size);
                                for _ in 0..batch_size {
                                    let measurement = rng.random_bool(0.5);
                                    let nonce: [u8; 16] = rng.random();
                                    let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &nonce).unwrap();
                                    reports.push((public_share, nonce, input_shares));
                                }
                                b.iter(|| {
                                    for (public_share, nonce, input_shares) in &reports {
                                        let mut states = Vec::with_capacity(NUM_AGGREGATORS);
                                        let mut shares = Vec::with_capacity(NUM_AGGREGATORS);
                                        for (agg_id, input_share) in input_shares.iter().enumerate() {
                                            let (state, share) = vdaf.prepare_init(
                                                &VERIFY_KEY,
                                                CONTEXT,
                                                agg_id,
                                                &(),
                                                nonce,
                                                public_share,
                                                input_share,
                                            ).unwrap();
                                            states.push(state);
                                            shares.push(share);
                                        }
                                        let mut finished = vec![false; NUM_AGGREGATORS];
                                        let mut _output_shares = vec![None; NUM_AGGREGATORS];
                                        let mut states = states;
                                        let mut shares = shares;
                                        while !finished.iter().all(|&f| f) {
                                            let prep_msg = vdaf.prepare_shares_to_prepare_message(CONTEXT, &(), shares.clone()).unwrap();
                                            for agg_id in 0..NUM_AGGREGATORS {
                                                if !finished[agg_id] {
                                                    match vdaf.prepare_next(CONTEXT, states[agg_id].clone(), prep_msg.clone()).unwrap() {
                                                        PrepareTransition::Continue(new_state, new_share) => {
                                                            states[agg_id] = new_state;
                                                            shares[agg_id] = new_share;
                                                        }
                                                        PrepareTransition::Finish(out_share) => {
                                                            finished[agg_id] = true;
                                                            _output_shares[agg_id] = Some(out_share);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            } else {
                                // Use Prio3SumVec for longer vectors
                                let vdaf = Prio3SumVec::new_sum_vec(NUM_AGGREGATORS as u8, 1, vec_len, vec_len).unwrap();
                                let mut reports = Vec::with_capacity(batch_size);
                                for _ in 0..batch_size {
                                    let measurement: Vec<u128> = (0..vec_len).map(|_| rng.random_range(0..2)).collect();
                                    let nonce: [u8; 16] = rng.random();
                                    let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &nonce).unwrap();
                                    reports.push((public_share, nonce, input_shares));
                                }
                                b.iter(|| {
                                    for (public_share, nonce, input_shares) in &reports {
                                        let mut states = Vec::with_capacity(NUM_AGGREGATORS);
                                        let mut shares = Vec::with_capacity(NUM_AGGREGATORS);
                                        for (agg_id, input_share) in input_shares.iter().enumerate() {
                                            let (state, share) = vdaf.prepare_init(
                                                &VERIFY_KEY,
                                                CONTEXT,
                                                agg_id,
                                                &(),
                                                nonce,
                                                public_share,
                                                input_share,
                                            ).unwrap();
                                            states.push(state);
                                            shares.push(share);
                                        }
                                        let mut finished = vec![false; NUM_AGGREGATORS];
                                        let mut _output_shares = vec![None; NUM_AGGREGATORS];
                                        let mut states = states;
                                        let mut shares = shares;
                                        while !finished.iter().all(|&f| f) {
                                            let prep_msg = vdaf.prepare_shares_to_prepare_message(CONTEXT, &(), shares.clone()).unwrap();
                                            for agg_id in 0..NUM_AGGREGATORS {
                                                if !finished[agg_id] {
                                                    match vdaf.prepare_next(CONTEXT, states[agg_id].clone(), prep_msg.clone()).unwrap() {
                                                        PrepareTransition::Continue(new_state, new_share) => {
                                                            states[agg_id] = new_state;
                                                            shares[agg_id] = new_share;
                                                        }
                                                        PrepareTransition::Finish(out_share) => {
                                                            finished[agg_id] = true;
                                                            _output_shares[agg_id] = Some(out_share);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            );
        }
    }
    group.finish();
}

#[cfg(feature = "experimental")]
fn aggregation_bench(c: &mut Criterion, ptype: ProtocolType, group_name: &str, seed: u64) {
    let mut group = c.benchmark_group(group_name);
    group.sample_size(10);
    for &batch_size in &BATCH_SIZES {
        for &vec_len in &VEC_LEN {
            group.bench_with_input(
                BenchmarkId::from_parameter(format!("batch_{}_vec_{}", batch_size, vec_len)), 
                &(batch_size, vec_len), 
                |b, &(batch_size, vec_len)| {
                    let mut rng = StdRng::seed_from_u64(seed);
                    match ptype {
                        ProtocolType::Prio2 => {
                            let vdaf = Prio2::new(vec_len).unwrap();
                            let mut output_shares = Vec::with_capacity(batch_size);
                            for _ in 0..batch_size {
                                let measurement: Vec<u32> = (0..vec_len).map(|i| (i & 1) as u32).collect();
                        let nonce: [u8; 16] = rng.random();
                        let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &nonce).unwrap();
                        let mut prep_states = Vec::with_capacity(NUM_AGGREGATORS);
                        let mut prep_shares = Vec::with_capacity(NUM_AGGREGATORS);
                        for (agg_id, input_share) in input_shares.iter().enumerate() {
                            let (state, share) = vdaf.prepare_init(
                                &VERIFY_KEY,
                                CONTEXT,
                                agg_id,
                                &(),
                                &nonce,
                                &public_share,
                                input_share,
                            ).unwrap();
                            prep_states.push(state);
                            prep_shares.push(share);
                        }
                        // For Prio2, we need to run the full preparation to get output shares
                        let prep_msg = vdaf.prepare_shares_to_prepare_message(CONTEXT, &(), prep_shares).unwrap();
                        for (agg_id, state) in prep_states.into_iter().enumerate() {
                            let out_share = match vdaf.prepare_next(CONTEXT, state, prep_msg.clone()).unwrap() {
                                PrepareTransition::Finish(out_share) => out_share,
                                _ => panic!("unexpected transition"),
                            };
                            if agg_id == 0 {
                                output_shares.push(out_share);
                            }
                        }
                    }
                    b.iter(|| {
                        let _ = vdaf.aggregate(&(), output_shares.iter().cloned()).unwrap();
                    });
                }
                ProtocolType::Prio3 => {
                    if vec_len == 1 {
                        // Use Prio3Count for single boolean values
                        let vdaf = Prio3Count::new_count(NUM_AGGREGATORS as u8).unwrap();
                        let mut output_shares = Vec::with_capacity(batch_size);
                        for _ in 0..batch_size {
                            let measurement = rng.random_bool(0.5);
                            let nonce: [u8; 16] = rng.random();
                            let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &nonce).unwrap();
                            let mut prep_states = Vec::with_capacity(NUM_AGGREGATORS);
                            let mut prep_shares = Vec::with_capacity(NUM_AGGREGATORS);
                            for (agg_id, input_share) in input_shares.iter().enumerate() {
                                let (state, share) = vdaf.prepare_init(
                                    &VERIFY_KEY,
                                    CONTEXT,
                                    agg_id,
                                    &(),
                                    &nonce,
                                    &public_share,
                                    input_share,
                                ).unwrap();
                                prep_states.push(state);
                                prep_shares.push(share);
                            }
                            let prep_msg = vdaf.prepare_shares_to_prepare_message(CONTEXT, &(), prep_shares).unwrap();
                            for (agg_id, state) in prep_states.into_iter().enumerate() {
                                let out_share = match vdaf.prepare_next(CONTEXT, state, prep_msg.clone()).unwrap() {
                                    PrepareTransition::Finish(out_share) => out_share,
                                    _ => panic!("unexpected transition"),
                                };
                                if agg_id == 0 {
                                    output_shares.push(out_share);
                                }
                            }
                        }
                        b.iter(|| {
                            let _ = vdaf.aggregate(&(), output_shares.iter().cloned()).unwrap();
                        });
                    } else {
                        // Use Prio3SumVec for longer vectors
                        let vdaf = Prio3SumVec::new_sum_vec(NUM_AGGREGATORS as u8, 1, vec_len, vec_len).unwrap();
                        let mut output_shares = Vec::with_capacity(batch_size);
                        for _ in 0..batch_size {
                            let measurement: Vec<u128> = (0..vec_len).map(|_| rng.random_range(0..2)).collect();
                            let nonce: [u8; 16] = rng.random();
                            let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &nonce).unwrap();
                            let mut prep_states = Vec::with_capacity(NUM_AGGREGATORS);
                            let mut prep_shares = Vec::with_capacity(NUM_AGGREGATORS);
                            for (agg_id, input_share) in input_shares.iter().enumerate() {
                                let (state, share) = vdaf.prepare_init(
                                    &VERIFY_KEY,
                                    CONTEXT,
                                    agg_id,
                                    &(),
                                    &nonce,
                                    &public_share,
                                    input_share,
                                ).unwrap();
                                prep_states.push(state);
                                prep_shares.push(share);
                            }
                            let prep_msg = vdaf.prepare_shares_to_prepare_message(CONTEXT, &(), prep_shares).unwrap();
                            for (agg_id, state) in prep_states.into_iter().enumerate() {
                                let out_share = match vdaf.prepare_next(CONTEXT, state, prep_msg.clone()).unwrap() {
                                    PrepareTransition::Finish(out_share) => out_share,
                                    _ => panic!("unexpected transition"),
                                };
                                if agg_id == 0 {
                                    output_shares.push(out_share);
                                }
                            }
                        }
                        b.iter(|| {
                            let _ = vdaf.aggregate(&(), output_shares.iter().cloned()).unwrap();
                        });
                    }
                }
            }
        });
        }
    }
    group.finish();
}

#[cfg(feature = "experimental")]
pub fn prio2_server_count_verification(c: &mut Criterion) {
    verification_bench(c, ProtocolType::Prio2, "prio2_server_count_verification", 1001);
}
#[cfg(feature = "experimental")]
pub fn prio3_server_count_verification(c: &mut Criterion) {
    verification_bench(c, ProtocolType::Prio3, "prio3_server_count_verification", 1002);
}
#[cfg(feature = "experimental")]
pub fn prio2_server_count_aggregation(c: &mut Criterion) {
    aggregation_bench(c, ProtocolType::Prio2, "prio2_server_count_aggregation", 2001);
}
#[cfg(feature = "experimental")]
pub fn prio3_server_count_aggregation(c: &mut Criterion) {
    aggregation_bench(c, ProtocolType::Prio3, "prio3_server_count_aggregation", 2002);
}



#[cfg(feature = "experimental")]
criterion_group!(benches,
    prio2_server_count_verification,
    prio3_server_count_verification,
    prio2_server_count_aggregation,
    prio3_server_count_aggregation,
);

criterion_main!(benches); 
use prio::vdaf::prio2::Prio2;
use prio::vdaf::prio3::{Prio3Count, Prio3SumVec};
use prio::vdaf::{Client, Aggregator, PrepareTransition};
use prio::codec::Encode;
use bytesize::ByteSize;

const CONTEXT: &[u8] = b"test context";
const NONCE: [u8; 16] = [0u8; 16];
const VERIFY_KEY: [u8; 32] = [0u8; 32];

fn measure_prio2_sizes(input_len: usize) {
    let vdaf = Prio2::new(input_len).unwrap();
    let measurement: Vec<u32> = (0..input_len).map(|i| (i & 1) as u32).collect();
    
    let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &NONCE).unwrap();
    
    // Prio2 has leader and helper shares
    let leader_share = &input_shares[0];
    let helper_share = &input_shares[1];
    
    // For Prio2, the entire share is the encoding (no separate proof)
    let leader_encoding_size = leader_share.encoded_len().unwrap();
    let helper_encoding_size = helper_share.encoded_len().unwrap();
    
    println!("Prio2 (input_len={}):", input_len);
    println!("  Leader encoding: {}", ByteSize::b(leader_encoding_size as u64));
    println!("  Helper encoding: {}", ByteSize::b(helper_encoding_size as u64));
    println!("  Public share: {}", ByteSize::b(public_share.encoded_len().unwrap() as u64));
    println!("  Total: {}", ByteSize::b((leader_encoding_size + helper_encoding_size + public_share.encoded_len().unwrap()) as u64));
    println!();
}

fn measure_prio3_count_sizes() {
    let vdaf = Prio3Count::new_count(2).unwrap();
    let measurement = true;
    let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &NONCE).unwrap();
    
    // Prio3 has leader and helper shares
    let leader_share = &input_shares[0];
    let helper_share = &input_shares[1];
    
    // For Prio3, we need to separate measurement share from proof data
    let (leader_encoding_size, leader_proof_size) = match leader_share {
        prio::vdaf::prio3::Prio3InputShare::Leader { measurement_share, proofs_share, joint_rand_blind } => {
            let encoding_size = measurement_share.len() * std::mem::size_of::<prio::field::Field64>();
            let proof_size = proofs_share.len() * std::mem::size_of::<prio::field::Field64>();
            let blind_size = joint_rand_blind.as_ref().map_or(0, |blind| blind.encoded_len().unwrap());
            (encoding_size, proof_size + blind_size)
        }
        _ => panic!("Expected leader share"),
    };
    
    let (helper_encoding_size, helper_proof_size) = match helper_share {
        prio::vdaf::prio3::Prio3InputShare::Helper { meas_and_proofs_share, joint_rand_blind } => {
            // For helper, the seed generates both measurement and proof shares
            // We can't easily separate them, so we'll show the total size
            let total_size = meas_and_proofs_share.encoded_len().unwrap();
            let blind_size = joint_rand_blind.as_ref().map_or(0, |blind| blind.encoded_len().unwrap());
            (total_size, blind_size)
        }
        _ => panic!("Expected helper share"),
    };
    
    println!("Prio3Count:");
    println!("  Leader encoding: {}, proof: {}", ByteSize::b(leader_encoding_size as u64), ByteSize::b(leader_proof_size as u64));
    println!("  Helper encoding: {}, proof: {}", ByteSize::b(helper_encoding_size as u64), ByteSize::b(helper_proof_size as u64));
    println!("  Public share: {}", ByteSize::b(public_share.encoded_len().unwrap() as u64));
    println!("  Total: {}", ByteSize::b((leader_encoding_size + leader_proof_size + helper_encoding_size + helper_proof_size + public_share.encoded_len().unwrap()) as u64));
    println!();
}

fn measure_prio3_sumvec_sizes(input_len: usize) {
    let vdaf = Prio3SumVec::new_sum_vec(2, 1, input_len, input_len).unwrap();
    let measurement: Vec<u128> = (0..input_len).map(|_| 1).collect();
    let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &NONCE).unwrap();
    
    // Prio3 has leader and helper shares
    let leader_share = &input_shares[0];
    let helper_share = &input_shares[1];
    
    // For Prio3, we need to separate measurement share from proof data
    let (leader_encoding_size, leader_proof_size) = match leader_share {
        prio::vdaf::prio3::Prio3InputShare::Leader { measurement_share, proofs_share, joint_rand_blind } => {
            let encoding_size = measurement_share.len() * std::mem::size_of::<prio::field::Field128>();
            let proof_size = proofs_share.len() * std::mem::size_of::<prio::field::Field128>();
            let blind_size = joint_rand_blind.as_ref().map_or(0, |blind| blind.encoded_len().unwrap());
            (encoding_size, proof_size + blind_size)
        }
        _ => panic!("Expected leader share"),
    };
    
    let (helper_encoding_size, helper_proof_size) = match helper_share {
        prio::vdaf::prio3::Prio3InputShare::Helper { meas_and_proofs_share, joint_rand_blind } => {
            // For helper, the seed generates both measurement and proof shares
            // We can't easily separate them, so we'll show the total size
            let total_size = meas_and_proofs_share.encoded_len().unwrap();
            let blind_size = joint_rand_blind.as_ref().map_or(0, |blind| blind.encoded_len().unwrap());
            (total_size, blind_size)
        }
        _ => panic!("Expected helper share"),
    };
    
    println!("Prio3SumVec (len={}):", input_len);
    println!("  Leader encoding: {}, proof: {}", ByteSize::b(leader_encoding_size as u64), ByteSize::b(leader_proof_size as u64));
    println!("  Helper encoding: {}, proof: {}", ByteSize::b(helper_encoding_size as u64), ByteSize::b(helper_proof_size as u64));
    println!("  Public share: {}", ByteSize::b(public_share.encoded_len().unwrap() as u64));
    println!("  Total: {}", ByteSize::b((leader_encoding_size + leader_proof_size + helper_encoding_size + helper_proof_size + public_share.encoded_len().unwrap()) as u64));
    println!();
}

fn measure_prio2_server_communication_batch(input_len: usize, num_clients: usize) -> (usize, usize) {
    let vdaf = Prio2::new(input_len).unwrap();
    let mut total_helper_to_leader = 0;
    let mut total_leader_to_helper = 0;
    for _ in 0..num_clients {
        let measurement: Vec<u32> = (0..input_len).map(|i| (i & 1) as u32).collect();
        let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &NONCE).unwrap();
        let mut prepare_shares = Vec::new();
        for (agg_id, input_share) in input_shares.iter().enumerate() {
            let (_, prepare_share) = vdaf.prepare_init(
                &VERIFY_KEY,
                CONTEXT,
                agg_id,
                &(),
                &NONCE,
                &public_share,
                input_share,
            ).unwrap();
            prepare_shares.push(prepare_share);
        }
        let prepare_message = vdaf.prepare_shares_to_prepare_message(CONTEXT, &(), prepare_shares.clone()).unwrap();
        total_helper_to_leader += prepare_shares[1].encoded_len().unwrap();
        total_leader_to_helper += prepare_message.encoded_len().unwrap();
    }
    (total_helper_to_leader, total_leader_to_helper)
}

fn measure_prio3_count_server_communication_batch(num_clients: usize) -> (usize, usize, usize) {
    let vdaf = Prio3Count::new_count(2).unwrap();
    let mut total_helper_to_leader = 0;
    let mut total_leader_to_helper = 0;
    let mut total_rounds = 0;
    for _ in 0..num_clients {
        let measurement = true;
        let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &NONCE).unwrap();
        let mut states = Vec::with_capacity(2);
        let mut shares = Vec::with_capacity(2);
        for (agg_id, input_share) in input_shares.iter().enumerate() {
            let (state, share) = vdaf.prepare_init(
                &VERIFY_KEY,
                CONTEXT,
                agg_id,
                &(),
                &NONCE,
                &public_share,
                input_share,
            ).unwrap();
            states.push(state);
            shares.push(share);
        }
        let mut round = 0;
        while !states.is_empty() {
            round += 1;
            total_helper_to_leader += shares[1].encoded_len().unwrap();
            let prepare_message = vdaf.prepare_shares_to_prepare_message(CONTEXT, &(), shares.clone()).unwrap();
            total_leader_to_helper += prepare_message.encoded_len().unwrap();
            let mut new_states = Vec::new();
            let mut new_shares = Vec::new();
            let mut finished = false;
            for (_agg_id, state) in states.into_iter().enumerate() {
                match vdaf.prepare_next(CONTEXT, state, prepare_message.clone()).unwrap() {
                    PrepareTransition::Continue(new_state, new_share) => {
                        new_states.push(new_state);
                        new_shares.push(new_share);
                    }
                    PrepareTransition::Finish(_) => {
                        finished = true;
                    }
                }
            }
            if finished {
                break;
            }
            states = new_states;
            shares = new_shares;
        }
        total_rounds += round;
    }
    (total_helper_to_leader, total_leader_to_helper, total_rounds)
}

fn measure_prio3_sumvec_server_communication_batch(input_len: usize, num_clients: usize) -> (usize, usize, usize) {
    let vdaf = Prio3SumVec::new_sum_vec(2, 1, input_len, input_len).unwrap();
    let mut total_helper_to_leader = 0;
    let mut total_leader_to_helper = 0;
    let mut total_rounds = 0;
    for _ in 0..num_clients {
        let measurement: Vec<u128> = (0..input_len).map(|_| 1).collect();
        let (public_share, input_shares) = vdaf.shard(CONTEXT, &measurement, &NONCE).unwrap();
        let mut states = Vec::with_capacity(2);
        let mut shares = Vec::with_capacity(2);
        for (agg_id, input_share) in input_shares.iter().enumerate() {
            let (state, share) = vdaf.prepare_init(
                &VERIFY_KEY,
                CONTEXT,
                agg_id,
                &(),
                &NONCE,
                &public_share,
                input_share,
            ).unwrap();
            states.push(state);
            shares.push(share);
        }
        let mut round = 0;
        while !states.is_empty() {
            round += 1;
            total_helper_to_leader += shares[1].encoded_len().unwrap();
            let prepare_message = vdaf.prepare_shares_to_prepare_message(CONTEXT, &(), shares.clone()).unwrap();
            total_leader_to_helper += prepare_message.encoded_len().unwrap();
            let mut new_states = Vec::new();
            let mut new_shares = Vec::new();
            let mut finished = false;
            for (_agg_id, state) in states.into_iter().enumerate() {
                match vdaf.prepare_next(CONTEXT, state, prepare_message.clone()).unwrap() {
                    PrepareTransition::Continue(new_state, new_share) => {
                        new_states.push(new_state);
                        new_shares.push(new_share);
                    }
                    PrepareTransition::Finish(_) => {
                        finished = true;
                    }
                }
            }
            if finished {
                break;
            }
            states = new_states;
            shares = new_shares;
        }
        total_rounds += round;
    }
    (total_helper_to_leader, total_leader_to_helper, total_rounds)
}

#[test]
fn measure_sizes() {
    println!("=== Client Size Measurements ===\n");
    
    // Test Prio2
    for &input_len in &[1, 5, 10, 25, 50, 75, 100] {
        measure_prio2_sizes(input_len);
    }
    
    // Test Prio3Count (for single boolean values)
    measure_prio3_count_sizes();
    
    // Test Prio3SumVec (for vectors)
    for &input_len in &[5, 10, 25, 50, 75, 100] {
        measure_prio3_sumvec_sizes(input_len);
    }
}

#[test]
fn measure_server_communication() {
    println!("=== Server-to-Server Communication Measurements (input_len=10, varying num_clients) ===\n");
    let input_len = 10;
    let num_clients_list = [1, 100, 10000];
    for &num_clients in &num_clients_list {
        // Prio2
        let (helper_to_leader, leader_to_helper) = measure_prio2_server_communication_batch(input_len, num_clients);
        println!("Prio2 | num_clients={}:", num_clients);
        println!("  Helper → Leader: {}", ByteSize::b(helper_to_leader as u64));
        println!("  Leader → Helper: {}", ByteSize::b(leader_to_helper as u64));
        println!("  Total: {}\n", ByteSize::b((helper_to_leader + leader_to_helper) as u64));

        // Prio3Count
        let (helper_to_leader, leader_to_helper, total_rounds) = measure_prio3_count_server_communication_batch(num_clients);
        println!("Prio3Count | num_clients={}:", num_clients);
        println!("  Helper → Leader: {}", ByteSize::b(helper_to_leader as u64));
        println!("  Leader → Helper: {}", ByteSize::b(leader_to_helper as u64));
        println!("  Total: {}", ByteSize::b((helper_to_leader + leader_to_helper) as u64));
        println!("  Avg rounds/report: {:.2}\n", total_rounds as f64 / num_clients as f64);

        // Prio3SumVec
        let (helper_to_leader, leader_to_helper, total_rounds) = measure_prio3_sumvec_server_communication_batch(input_len, num_clients);
        println!("Prio3SumVec | num_clients={}:", num_clients);
        println!("  Helper → Leader: {}", ByteSize::b(helper_to_leader as u64));
        println!("  Leader → Helper: {}", ByteSize::b(leader_to_helper as u64));
        println!("  Total: {}", ByteSize::b((helper_to_leader + leader_to_helper) as u64));
        println!("  Avg rounds/report: {:.2}\n", total_rounds as f64 / num_clients as f64);
    }
    println!();
} 

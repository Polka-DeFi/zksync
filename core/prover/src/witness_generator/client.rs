use crate::witness_generator::server;
use crate::ProverData;
use bellman::groth16;
use std::str::FromStr;
use std::sync::Mutex;
use std::{fmt, net, thread, time};

#[derive(Debug)]
pub struct FullBabyProof {
    proof: bellman::groth16::Proof<models::node::Engine>,
    inputs: [models::node::Fr; 1],
    public_data: Vec<u8>,
}

#[derive(Debug)]
pub struct ApiClient {
    register_url: String,
    block_to_prove_url: String,
    working_on_url: String,
    prover_data_url: String,
    publish_url: String,
    worker: String,
    current_prover_run_id: Mutex<i32>,
    current_proving_block: Mutex<i64>,
}

#[derive(Debug)]
pub enum Error {
    Default,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} error", "default")
    }
}

impl ApiClient {
    pub fn new(base_url: &str, worker: &str) -> Self {
        if worker == "" {
            panic!("worker name cannot be empty")
        }
        ApiClient {
            register_url: format!("{}/register", base_url),
            block_to_prove_url: format!("{}/block_to_prove", base_url),
            working_on_url: format!("{}/working_on", base_url),
            prover_data_url: format!("{}/prover_data", base_url),
            publish_url: format!("{}/publish", base_url),
            worker: worker.to_string(),
            current_prover_run_id: Mutex::new(0),
            current_proving_block: Mutex::new(0),
        }
    }

    pub fn register_prover(&self) -> Result<i32, Error> {
        // TODO: handle errors
        let client = reqwest::Client::new();
        let mut res = client
            .post(&self.register_url)
            .json(&server::ProverReq {
                name: self.worker.clone(),
            })
            .send()
            .unwrap();
        let id = i32::from_str(&res.text().unwrap()).unwrap();
        Ok(id)
    }
}

impl crate::ApiClient for ApiClient {
    fn block_to_prove(&self) -> Result<Option<i64>, String> {
        // TODO: handle errors
        let mut current_prover_run_id = self.current_prover_run_id.lock().unwrap();
        let mut current_proving_block = self.current_proving_block.lock().unwrap();
        let client = reqwest::Client::new();
        let mut res = client
            .get(&self.block_to_prove_url)
            .json(&server::ProverReq {
                name: self.worker.clone(),
            })
            .send()
            .unwrap();
        let text = res.text().unwrap();
        let res: server::BlockToProveRes = serde_json::from_str(&text).unwrap();
        if res.block != 0 {
            *current_prover_run_id = res.prover_run_id;
            *current_proving_block = res.block;
            return Ok(Some(res.block));
        }
        Ok(None)
    }

    fn working_on(&self, block: i64) {
        // TODO: handle errors
        let client = reqwest::Client::new();
        client
            .post(&self.working_on_url)
            .json(&server::WorkingOnReq {
                prover_run_id: *self.current_prover_run_id.lock().unwrap(),
            })
            .send()
            .unwrap();
    }

    fn prover_data(&self, timeout: time::Duration) -> Result<ProverData, String> {
        let block = self.current_proving_block.lock().unwrap();
        println!("client::prover_data {}", *block);
        let client = reqwest::Client::new();
        // TODO: whats the idiomatic way of cancallation by timeout.
        let now = time::SystemTime::now();
        while now.elapsed().unwrap() < timeout {
            let mut res = client
                .get(&self.prover_data_url)
                .json(&(*block))
                .send()
                .unwrap();
            let text = res.text().unwrap();
            let res: Option<ProverData> = serde_json::from_str(&text).unwrap();
            if let Some(res) = res {
                return Ok(res);
            }
            thread::sleep(time::Duration::from_secs(10));
        }

        Err("timeout".to_owned())
    }

    fn publish(
        &self,
        block: i64,
        proof: groth16::Proof<models::node::Engine>,
        public_data_commitment: models::node::Fr,
    ) -> Result<(), String> {
        let full_proof = FullBabyProof {
            proof,
            inputs: [public_data_commitment],
            public_data: vec![0 as u8; 10],
        };

        let encoded = encode_proof(&full_proof);

        let client = reqwest::Client::new();
        let res = client
            .post(&self.publish_url)
            .json(&server::PublishReq {
                block: block as u32,
                proof: encoded,
            })
            .send()
            .unwrap();
        if res.status() != reqwest::StatusCode::OK {
            return Err("err".to_owned());
        }

        Ok(())
    }
}

fn encode_proof(proof: &FullBabyProof) -> models::EncodedProof {
    // proof
    // pub a: E::G1Affine,
    // pub b: E::G2Affine,
    // pub c: E::G1Affine

    let (a_x, a_y) = models::primitives::serialize_g1_for_ethereum(proof.proof.a);

    let ((b_x_0, b_x_1), (b_y_0, b_y_1)) =
        models::primitives::serialize_g2_for_ethereum(proof.proof.b);

    let (c_x, c_y) = models::primitives::serialize_g1_for_ethereum(proof.proof.c);

    [a_x, a_y, b_x_0, b_x_1, b_y_0, b_y_1, c_x, c_y]
}

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::message::{Request, Response, InvalidRequest, PutResponseBody, PutResponse};


// pub trait JobApi {
//     fn put(&mut self, queue: &str, job: &serde_json::Value, priority: usize) {

//     }
// }

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Job {
    pub priority: usize,
    pub payload: serde_json::Value,
}

impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.priority.partial_cmp(&other.priority)
    }
}


impl Ord for Job {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}


#[derive(Debug, Default)]
pub struct AppState {
    pub last_job_id: AtomicUsize,
    pub queues: HashMap<String, BinaryHeap<Job>>,
    pub job_ids_by_peer: HashMap<SocketAddr, HashSet<usize>>
}

impl AppState {
    pub fn next_job_id(&self) -> usize {
        self.last_job_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn put_job(&mut self, queue: &str, job: &serde_json::Value, priority: usize) -> PutResponse {
        let job_id = self.next_job_id();
        let job = Job {
            priority,
            payload: job.clone(),
        };

        let queue = 
            self
            .queues
            .entry(queue.to_string())
            .or_insert(Default::default());

        queue.push(job);

        PutResponse::Ok(PutResponseBody { id: job_id })
    }

    pub fn process_request(&mut self, _peer: &SocketAddr,  request: &Request) -> Response {

        match request {
            Request::Put(put_request) => {
                let response = self.put_job(&put_request.queue, &put_request.job, put_request.pri);
                Response::Put(response)
            },
            _ => {
                Response::Invalid(InvalidRequest { status: "error".to_string(), error: "foo".to_string() })
            }
        }
    }

}
use serde::{Serialize, Deserialize};


#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase", tag = "request")]
pub enum Request {

    Put(PutRequest),
    Get(GetRequest),
    Delete(DeleteRequest),
    Abort(AbortRequest),
    Invalid,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", tag = "status")]
pub enum ResponseInner<T> {
    NoJob,
    Ok(T)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Response {
    Put(PutResponse),
    Get(GetResponse),
    Delete(DeleteResponse),
    Abort(AbortResponse),
    Invalid(InvalidRequest)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InvalidRequest {
    pub error: String,
    pub status: String
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PutRequest {
    pub queue: String,
    pub pri: usize,
    pub job: serde_json::Value
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PutResponseBody {
    pub id: usize
}

pub type PutResponse = ResponseInner<PutResponseBody>;



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetRequest {
    pub queues: Vec<String>,
    pub wait: Option<bool>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetResponseBody {
    pub id: usize,
    pub job: serde_json::Value,
    pub pri: usize,
    pub queue: String,
}

pub type GetResponse = ResponseInner<GetResponseBody>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteRequest {
    pub id: usize
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteResponseBody;

pub type DeleteResponse = ResponseInner<DeleteResponseBody>;

pub type AbortRequest = DeleteRequest;
pub type AbortResponse = DeleteResponse;



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serde() {
        let response = Response::Invalid(InvalidRequest { status: "error".to_string(), error: "foo".to_string() });
        let serialized = serde_json::to_string_pretty(&response).unwrap();
        println!("{}", serialized);
    }
}
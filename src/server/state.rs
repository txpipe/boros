use spec::boros::v1::{state_service_server::StateService, FetchStateRequest, FetchStateResponse};

pub struct StateServiceImpl {}

impl StateServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl StateService for StateServiceImpl {
    async fn fetch_state(
        &self,
        _request: tonic::Request<FetchStateRequest>,
    ) -> std::result::Result<tonic::Response<FetchStateResponse>, tonic::Status> {
        todo!()
    }
}

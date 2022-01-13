use crate::sync::oneshot;

pub struct Request<ServiceRequest, ServiceResponse> {
    pub request: ServiceRequest,
    pub response_sender: oneshot::Sender<ServiceResponse>,
}

impl<ServiceRequest, ServiceResponse> Request<ServiceRequest, ServiceResponse> {
    pub fn new(request: ServiceRequest) -> (Self, oneshot::Receiver<ServiceResponse>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                request,
                response_sender: tx,
            },
            rx,
        )
    }
}

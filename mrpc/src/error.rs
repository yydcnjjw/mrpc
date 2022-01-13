#[derive(serde::Deserialize, serde::Serialize)]
pub struct ServiceError {
    description: String,
    source: Option<Box<ServiceError>>,
}

impl ServiceError {
    pub fn new<T>(e: &T) -> ServiceError
    where
        T: ?Sized + std::error::Error,
    {
        ServiceError {
            description: e.to_string(),
            source: e.source().map(|s| Box::new(ServiceError::new(s))),
        }
    }
}

impl std::error::Error for ServiceError {
    fn source(&self) -> Option<&(dyn 'static + std::error::Error)> {
        self.source
            .as_ref()
            .map(|s| &**s as &(dyn 'static + std::error::Error))
    }

    fn description(&self) -> &str {
        &self.description
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description)
    }
}

impl std::fmt::Debug for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description)
    }
}

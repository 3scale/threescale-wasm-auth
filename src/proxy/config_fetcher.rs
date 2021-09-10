pub enum FetcherState {
    Inactive,
    FetchingRules(u32, String),
    RulesFetched,
    FetchingConfigs(u32),
    Error(Box<dyn std::error::Error + Send + Sync>),
}
